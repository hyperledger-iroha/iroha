//! Authenticated, process-local spooling for bounded confidential chunks.
//!
//! Version 1 stores equally sized chunks in write-once slots in an unlinked Unix temporary file.
//! Every slot is independently protected with XChaCha20-Poly1305. The file is never reopenable
//! through this API: the live file descriptor, zeroizing key, immutable layout, context, and arena
//! nonce prefix move together from [`ConfidentialSpoolWriterV1`] to
//! [`ConfidentialSpoolSnapshotV1`].
//!
//! This module deliberately makes narrower claims than encrypted storage. It does not claim secure
//! deletion, swap exclusion, core-dump exclusion, page-cache erasure, or an RSS bound. The snapshot
//! digest is deterministic for the encrypted snapshot, but is neither an authentication authority
//! nor a hiding commitment. A caller must bind it into its own authenticated protocol transcript
//! before relying on it. An unlinked descriptor and the process key memory survive `fork`; callers
//! must not fork while a spool is live. Close-on-exec, ptrace/procfs isolation, crash behavior,
//! core policy, and swap remain outside this source guarantee. The retained key and owned plaintext
//! chunks are zeroized, but this source does not guarantee erasure of compiler/register temporaries
//! or derived Poly1305 state. In particular, the current dependency features do not enable
//! `poly1305/zeroize`; this primitive alone cannot satisfy release secret-lifecycle evidence.
//!
//! # Canonical framing
//!
//! Integers below are unsigned, big-endian `u64` values. The exact per-slot AAD is, in order:
//!
//! ```text
//! AAD_DOMAIN
//! || LAYOUT_DOMAIN
//! || slot_count || plaintext_len || ciphertext_record_len || file_len
//! || context_digest[32]
//! || arena_id[16]
//! || slot
//! || derived_coordinate[32]
//! || plaintext_len || ciphertext_record_len
//! ```
//!
//! The 24-byte XChaCha nonce is exactly `arena_id[16] || slot`. The canonical
//! snapshot digest is BLAKE3 over `SNAPSHOT_DIGEST_DOMAIN`, the same immutable
//! layout/context-digest/arena prefix, and then, for every slot in numeric
//! order, `slot || derived_coordinate || ciphertext_record_len || ciphertext
//! || tag`. The public, non-authorizing derived coordinate is BLAKE3 over
//! `COORDINATE_DOMAIN`, the public nonsecret context digest, exact layout, and slot. The context
//! digest must bind the protocol/version/role and complete canonical slot-to-application-coordinate
//! mapping; reusing it after a mapping, order, or interpretation change is a caller error.

#[cfg(unix)]
use std::fs::{self, Metadata};
use std::{fs::File, io, path::Path};

use aead::{AeadInOut as _, KeyInit as _};
use chacha20poly1305::XChaCha20Poly1305;
use rand::rngs::OsRng;
use rand_core::TryRngCore as _;
use zeroize::{Zeroize as _, Zeroizing};

const AAD_DOMAIN_V1: &[u8] = b"iroha.confidential-spool.aad.v1\0";
const LAYOUT_DOMAIN_V1: &[u8] = b"iroha.confidential-spool.fixed-layout.v1\0";
const SNAPSHOT_DIGEST_DOMAIN_V1: &[u8] = b"iroha.confidential-spool.snapshot-digest.v1\0";
const COORDINATE_DOMAIN_V1: &[u8] = b"iroha.confidential-spool.coordinate.v1\0";
const KEY_BYTES_V1: usize = 32;
const ARENA_ID_BYTES_V1: usize = 16;
const NONCE_BYTES_V1: usize = 24;
const TAG_BYTES_V1: u64 = 16;
const CONTEXT_DIGEST_BYTES_V1: usize = 32;

/// Maximum supported number of fixed write-once slots in one V1 spool.
///
/// The limit covers all currently audited Phase23 layouts. Release integration
/// must still admit only its exact approved layout tuples.
pub const CONFIDENTIAL_SPOOL_MAX_SLOTS_V1: u64 = 466_560;

/// Maximum supported plaintext bytes in one V1 record (16 KiB).
pub const CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1: u64 = 16_384;

/// Maximum supported detached-file bytes in one V1 spool.
///
/// This is the audited 466,560-record Phase23 arena at 8,192 plaintext bytes
/// plus one 16-byte tag per record.
pub const CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1: u64 = 3_829_524_480;

/// Width of the semantic coordinate bound to every confidential spool slot.
pub const CONFIDENTIAL_SPOOL_COORDINATE_BYTES_V1: usize = 32;

/// Errors returned by the V1 confidential spool.
///
/// Filesystem errors intentionally retain only a coarse operation label and [`io::ErrorKind`]. They
/// never retain a caller-supplied directory path or an underlying platform error string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfidentialSpoolErrorV1 {
    /// The layout must contain at least one slot.
    #[error("confidential spool slot count must be nonzero")]
    EmptyLayout,
    /// Each slot must contain at least one plaintext byte.
    #[error("confidential spool plaintext length must be nonzero")]
    EmptyChunk,
    /// A layout length, offset, framing size, or AAD size overflowed.
    #[error("confidential spool geometry overflow")]
    GeometryOverflow,
    /// The layout cannot be addressed by this process.
    #[error("confidential spool layout exceeds the process address space")]
    AddressSpaceExceeded,
    /// The immutable application context digest must not be all zero.
    #[error("confidential spool context digest must be nonzero")]
    InertContextDigest,
    /// A public V1 resource bound was exceeded.
    #[error("confidential spool limit exceeded for {0}")]
    LimitExceeded(&'static str),
    /// The plaintext length exceeds XChaCha20-Poly1305's per-message limit.
    #[error("confidential spool plaintext exceeds the cipher message limit")]
    CipherMessageLimit,
    /// A bounded internal allocation failed.
    #[error("confidential spool allocation failed for {0}")]
    Allocation(&'static str),
    /// The platform cannot provide the required unlink and descriptor checks.
    #[error("confidential spool V1 requires Unix descriptor semantics")]
    UnsupportedPlatform,
    /// A filesystem operation failed.
    #[error("confidential spool filesystem operation {operation} failed with kind {kind:?}")]
    FileOperation {
        /// Coarse, non-path operation label.
        operation: &'static str,
        /// Stable error classification without platform error text.
        kind: io::ErrorKind,
    },
    /// The temporary file was not an empty, private, single-link regular file.
    #[error("confidential spool temporary file failed the pre-unlink descriptor checks")]
    UnsafeTemporaryFile,
    /// The temporary pathname and live descriptor did not identify one inode.
    #[error("confidential spool temporary pathname did not match its descriptor")]
    TemporaryFileIdentityMismatch,
    /// The detached descriptor was not an empty, private, unlinked regular file.
    #[error("confidential spool descriptor failed the post-unlink checks")]
    UnsafeDetachedFile,
    /// Operating-system entropy was unavailable.
    #[error("confidential spool entropy was unavailable")]
    EntropyUnavailable,
    /// Entropy produced a known broken-source pattern.
    #[error("confidential spool entropy produced weak {0} material")]
    WeakEntropy(&'static str),
    /// A slot index was outside the immutable layout.
    #[error("confidential spool slot {slot} is outside slot count {slot_count}")]
    SlotOutOfRange {
        /// Rejected slot.
        slot: u64,
        /// Immutable slot count.
        slot_count: u64,
    },
    /// A write did not target the one exact next sequential slot.
    #[error("confidential spool expected write slot {expected}, got {actual}")]
    UnexpectedWriteSlot {
        /// Exact next slot accepted by the writer.
        expected: u64,
        /// Rejected caller slot.
        actual: u64,
    },
    /// A read expected a different immutable application context digest.
    #[error("confidential spool read context digest does not match the snapshot")]
    ContextDigestMismatch,
    /// A plaintext owner did not have the exact fixed chunk length.
    #[error("confidential spool chunk length {actual} differs from required {expected}")]
    ChunkLength {
        /// Required plaintext length.
        expected: u64,
        /// Supplied plaintext length.
        actual: u64,
    },
    /// Sealing was attempted before every write-once slot was filled.
    #[error("confidential spool is missing {remaining} slots")]
    Incomplete {
        /// Exact number of unfilled slots.
        remaining: u64,
    },
    /// The live file length differed from the immutable fixed layout.
    #[error("confidential spool file length {actual} differs from required {expected}")]
    FileLength {
        /// Required file length.
        expected: u64,
        /// Observed file length.
        actual: u64,
    },
    /// XChaCha20-Poly1305 rejected an encryption operation.
    #[error("confidential spool encryption failed")]
    Encryption,
    /// XChaCha20-Poly1305 rejected a record authentication operation.
    #[error("confidential spool record authentication failed")]
    Authentication,
    /// A previous cryptographic or I/O operation poisoned this handle.
    #[error("confidential spool handle is poisoned")]
    Poisoned,
}

/// Immutable fixed-record layout and application context for one V1 spool.
///
/// Constructing a layout performs all record, file, AAD, and process-address-space geometry checks
/// before filesystem or entropy effects. The public, nonsecret context digest must bind the
/// protocol/version/role and the complete canonical slot-to-application-coordinate mapping. It is
/// bound into every derived coordinate, record, and snapshot digest. This type exposes no
/// context-digest accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfidentialSpoolLayoutV1 {
    slot_count: u64,
    plaintext_len: u64,
    ciphertext_record_len: u64,
    file_len: u64,
    aad_len: usize,
    context_digest: [u8; CONTEXT_DIGEST_BYTES_V1],
}

impl ConfidentialSpoolLayoutV1 {
    /// Validate a fixed-record layout and its nonzero protocol-context digest.
    ///
    /// # Errors
    ///
    /// Returns an error for empty fields, arithmetic overflow, a public bound,
    /// the cipher message limit, or geometry this process cannot index.
    pub fn new_v1(
        slot_count: u64,
        plaintext_len: u64,
        context_digest: [u8; CONTEXT_DIGEST_BYTES_V1],
    ) -> Result<Self, ConfidentialSpoolErrorV1> {
        if slot_count == 0 {
            return Err(ConfidentialSpoolErrorV1::EmptyLayout);
        }
        if plaintext_len == 0 {
            return Err(ConfidentialSpoolErrorV1::EmptyChunk);
        }
        if context_digest.iter().all(|byte| *byte == 0) {
            return Err(ConfidentialSpoolErrorV1::InertContextDigest);
        }
        if slot_count > CONFIDENTIAL_SPOOL_MAX_SLOTS_V1 {
            return Err(ConfidentialSpoolErrorV1::LimitExceeded("slot count"));
        }
        // XChaCha20's 32-bit block counter permits fewer than u32::MAX
        // 64-byte message blocks. Keep this explicit even though the tighter
        // public record bound above currently dominates it.
        if plaintext_len / 64 >= u64::from(u32::MAX) {
            return Err(ConfidentialSpoolErrorV1::CipherMessageLimit);
        }
        if plaintext_len > CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1 {
            return Err(ConfidentialSpoolErrorV1::LimitExceeded(
                "plaintext record length",
            ));
        }

        let ciphertext_record_len = plaintext_len
            .checked_add(TAG_BYTES_V1)
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
        let file_len = slot_count
            .checked_mul(ciphertext_record_len)
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
        if file_len > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1 {
            return Err(ConfidentialSpoolErrorV1::LimitExceeded("file length"));
        }
        usize::try_from(slot_count).map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?;
        usize::try_from(plaintext_len)
            .map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?;
        usize::try_from(ciphertext_record_len)
            .map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?;
        let aad_len = AAD_DOMAIN_V1
            .len()
            .checked_add(LAYOUT_DOMAIN_V1.len())
            .and_then(|value| value.checked_add(4 * size_of::<u64>()))
            .and_then(|value| value.checked_add(CONTEXT_DIGEST_BYTES_V1))
            .and_then(|value| value.checked_add(ARENA_ID_BYTES_V1))
            .and_then(|value| value.checked_add(size_of::<u64>()))
            .and_then(|value| value.checked_add(CONFIDENTIAL_SPOOL_COORDINATE_BYTES_V1))
            .and_then(|value| value.checked_add(2 * size_of::<u64>()))
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;

        Ok(Self {
            slot_count,
            plaintext_len,
            ciphertext_record_len,
            file_len,
            aad_len,
            context_digest,
        })
    }

    /// Return the fixed number of write-once slots.
    pub const fn slot_count_v1(&self) -> u64 {
        self.slot_count
    }

    /// Return the exact plaintext bytes in every slot.
    pub const fn plaintext_len_v1(&self) -> u64 {
        self.plaintext_len
    }

    /// Return the exact on-disk bytes per slot, including the 16-byte tag.
    pub const fn ciphertext_record_len_v1(&self) -> u64 {
        self.ciphertext_record_len
    }

    /// Return the exact detached file length.
    pub const fn file_len_v1(&self) -> u64 {
        self.file_len
    }

    /// Return the exact persistent sequential-cursor bytes.
    ///
    /// This narrow accounting excludes Rust/allocator overhead, key and arena material, the live
    /// chunk/AAD/cipher state, filesystem and page cache, and all operating-system accounting.
    #[expect(
        clippy::unused_self,
        reason = "cursor accounting remains an instance-level layout query for API consistency"
    )]
    pub const fn writer_cursor_bytes_v1(&self) -> u64 {
        8
    }

    fn slot_index_v1(&self, slot: u64) -> Result<usize, ConfidentialSpoolErrorV1> {
        if slot >= self.slot_count {
            return Err(ConfidentialSpoolErrorV1::SlotOutOfRange {
                slot,
                slot_count: self.slot_count,
            });
        }
        usize::try_from(slot).map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)
    }

    fn slot_offset_v1(&self, slot: u64) -> Result<u64, ConfidentialSpoolErrorV1> {
        self.slot_index_v1(slot)?;
        slot.checked_mul(self.ciphertext_record_len)
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)
    }
}

/// Move-only, zeroizing owner for exactly one plaintext chunk.
///
/// [`Self::new_zeroed_v1`] is the only public constructor. It creates the final exact-size boxed
/// allocation without copying caller plaintext. The temporary zero-only `Vec` may be shrunk while
/// converting to the box, before callers can place secret bytes in it. Borrowing the slice cannot
/// prevent a caller from explicitly copying or forgetting secret bytes; the owner only guarantees
/// zeroization of the allocation it retains.
pub struct ConfidentialSpoolChunkV1 {
    bytes: Zeroizing<Box<[u8]>>,
    len: u64,
}

impl ConfidentialSpoolChunkV1 {
    /// Fallibly allocate an exact-size, zero-filled plaintext owner.
    ///
    /// # Errors
    ///
    /// Returns an error if the length cannot be represented or the fallible
    /// reserve step fails. The final zero-only conversion into an exact box can
    /// still follow the process allocator's abort policy.
    pub fn new_zeroed_v1(len: u64) -> Result<Self, ConfidentialSpoolErrorV1> {
        if len == 0 {
            return Err(ConfidentialSpoolErrorV1::EmptyChunk);
        }
        if len > CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1 {
            return Err(ConfidentialSpoolErrorV1::LimitExceeded(
                "plaintext chunk length",
            ));
        }
        let requested_len = len;
        let len =
            usize::try_from(len).map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| ConfidentialSpoolErrorV1::Allocation("plaintext chunk"))?;
        bytes.resize(len, 0);
        Ok(Self {
            bytes: Zeroizing::new(bytes.into_boxed_slice()),
            len: requested_len,
        })
    }

    /// Borrow the exact plaintext bytes without transferring ownership.
    pub fn as_slice_v1(&self) -> &[u8] {
        &self.bytes
    }

    /// Mutably borrow the exact plaintext bytes without transferring ownership.
    pub fn as_mut_slice_v1(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Return the exact plaintext length.
    pub fn len_v1(&self) -> u64 {
        self.len
    }
}

impl Drop for ConfidentialSpoolChunkV1 {
    fn drop(&mut self) {
        // Explicitly zero here as well as relying on `Zeroizing` so tests can
        // observe all success/error/unwind ownership paths at this boundary.
        self.bytes.zeroize();
        #[cfg(test)]
        {
            debug_assert!(self.bytes.iter().all(|byte| *byte == 0));
            TEST_ZEROIZED_CHUNK_DROPS_V1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Move-only writer for a fixed-layout authenticated confidential spool.
///
/// Slots must be written exactly once in strict numeric order. Dropping or explicitly
/// aborting the writer closes the already-unlinked file and zeroizes its key;
/// it does not make secure-deletion claims about operating-system storage.
pub struct ConfidentialSpoolWriterV1 {
    resources: Option<ConfidentialSpoolResourcesV1>,
    next_slot: u64,
}

impl ConfidentialSpoolWriterV1 {
    /// Create an empty spool in the caller-selected directory.
    ///
    /// On Unix this creates a named `0600` regular temporary file, compares the
    /// pathname and descriptor device/inode identities, explicitly unlinks it
    /// while empty, verifies `nlink == 0`, generates independently checked key
    /// and arena material, and only then sizes the file. Other platforms fail
    /// closed. A restrictive umask that prevents exact `0600` mode is rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when descriptor validation, unlink, sizing, or entropy generation fails.
    pub fn create_in_v1(
        directory: impl AsRef<Path>,
        layout: ConfidentialSpoolLayoutV1,
    ) -> Result<Self, ConfidentialSpoolErrorV1> {
        let mut entropy = OsEntropyV1;
        Self::create_in_with_entropy_v1(directory.as_ref(), layout, &mut entropy)
    }

    fn create_in_with_entropy_v1(
        directory: &Path,
        layout: ConfidentialSpoolLayoutV1,
        entropy: &mut impl EntropySourceV1,
    ) -> Result<Self, ConfidentialSpoolErrorV1> {
        let mut file = create_detached_empty_file_v1(directory)?;

        // Allocate first, then ask the entropy source to fill the final key
        // allocation directly. The secret is never assembled in a stack array.
        let mut key = Box::new(Zeroizing::new([0_u8; KEY_BYTES_V1]));
        entropy.fill_v1(&mut **key)?;
        if bytes_are_constant_v1(&key[..]) {
            return Err(ConfidentialSpoolErrorV1::WeakEntropy("key"));
        }
        let mut arena_id = [0_u8; ARENA_ID_BYTES_V1];
        entropy.fill_v1(&mut arena_id)?;
        if bytes_are_constant_v1(&arena_id) {
            return Err(ConfidentialSpoolErrorV1::WeakEntropy("arena"));
        }
        if key[..ARENA_ID_BYTES_V1] == arena_id {
            return Err(ConfidentialSpoolErrorV1::WeakEntropy(
                "key/arena separation",
            ));
        }
        file.size_v1(layout.file_len)?;

        Ok(Self {
            resources: Some(ConfidentialSpoolResourcesV1 {
                file,
                key,
                arena_id,
                layout,
            }),
            next_slot: 0,
        })
    }

    /// Encrypt and write one previously empty slot.
    ///
    /// Slot bounds, chunk length, and duplicate-slot checks are pure caller
    /// preflights and do not poison the writer. Once cryptographic processing
    /// begins, resources are removed from the handle; an error or unwind drops
    /// them permanently, while only complete success restores them.
    ///
    /// # Errors
    ///
    /// Returns an error for a pure preflight rejection, poisoned handle,
    /// encryption failure, or fixed-offset write failure.
    pub fn write_slot_v1(
        &mut self,
        slot: u64,
        mut chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        let resources = self
            .resources
            .as_ref()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?;
        resources.layout.slot_index_v1(slot)?;
        if slot != self.next_slot {
            return Err(ConfidentialSpoolErrorV1::UnexpectedWriteSlot {
                expected: self.next_slot,
                actual: slot,
            });
        }
        if chunk.len_v1() != resources.layout.plaintext_len {
            return Err(ConfidentialSpoolErrorV1::ChunkLength {
                expected: resources.layout.plaintext_len,
                actual: chunk.len_v1(),
            });
        }

        let mut resources = self
            .resources
            .take()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?;
        let result = write_slot_operation_v1(&mut resources, slot, &mut chunk);
        match result {
            Ok(()) => {
                self.next_slot = self
                    .next_slot
                    .checked_add(1)
                    .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
                self.resources = Some(resources);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Consume and authenticate every record, returning an immutable snapshot.
    ///
    /// Sealing requires all slots and the exact file length. It rereads slots in canonical numeric
    /// order, hashes the derived coordinate and original ciphertext/tag, and authenticates every
    /// record before finalizing the digest. Any I/O, length, allocation, or authentication error
    /// consumes and poisons the backing resources.
    ///
    /// # Errors
    ///
    /// Returns an error if slots are missing or the canonical authenticated seal pass fails.
    pub fn seal_v1(mut self) -> Result<ConfidentialSpoolSnapshotV1, ConfidentialSpoolErrorV1> {
        let slot_count = self
            .resources
            .as_ref()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?
            .layout
            .slot_count;
        let remaining = slot_count
            .checked_sub(self.next_slot)
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
        if remaining != 0 {
            return Err(ConfidentialSpoolErrorV1::Incomplete { remaining });
        }
        let mut resources = self
            .resources
            .take()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?;
        let digest = seal_operation_v1(&mut resources)?;
        Ok(ConfidentialSpoolSnapshotV1 {
            slot_count: resources.layout.slot_count,
            plaintext_len: resources.layout.plaintext_len,
            ciphertext_record_len: resources.layout.ciphertext_record_len,
            file_len: resources.layout.file_len,
            resources: Some(resources),
            digest,
        })
    }

    /// Consume the writer and abort its unlinked backing resources.
    pub fn abort_v1(self) {
        drop(self);
    }
}

/// Move-only immutable sealed handle for authenticated random reads.
///
/// The handle has no write, reopen, path, key, or file accessor. Dropping or
/// explicitly aborting it closes the unlinked file and zeroizes its key.
pub struct ConfidentialSpoolSnapshotV1 {
    resources: Option<ConfidentialSpoolResourcesV1>,
    digest: [u8; 32],
    slot_count: u64,
    plaintext_len: u64,
    ciphertext_record_len: u64,
    file_len: u64,
}

impl ConfidentialSpoolSnapshotV1 {
    /// Return the deterministic encrypted-snapshot digest.
    ///
    /// This value is non-secret and non-authorizing. It does not authenticate
    /// its own provenance and is not a hiding commitment.
    pub const fn snapshot_digest_v1(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Return the immutable slot count.
    pub const fn slot_count_v1(&self) -> u64 {
        self.slot_count
    }

    /// Return the immutable plaintext bytes per slot.
    pub const fn plaintext_len_v1(&self) -> u64 {
        self.plaintext_len
    }

    /// Return the immutable ciphertext bytes per record, including its tag.
    pub const fn ciphertext_record_len_v1(&self) -> u64 {
        self.ciphertext_record_len
    }

    /// Return the immutable detached-file length.
    pub const fn file_len_v1(&self) -> u64 {
        self.file_len
    }

    /// Authentically read one random-access slot into a zeroizing owner.
    ///
    /// Slot bounds and exact expected-context equality are pure caller preflights and do not poison
    /// the snapshot. The operation then removes the resources, derives the exact coordinate,
    /// revalidates the detached descriptor, reads the fixed-offset record, and authenticates it.
    /// Any operational error or unwind permanently drops the file and key; only success restores
    /// them.
    ///
    /// # Errors
    ///
    /// Returns an error for a preflight mismatch, poisoned handle, changed file
    /// geometry, failed read, allocation failure, or failed authentication.
    pub fn read_slot_v1(
        &mut self,
        slot: u64,
        expected_context_digest: [u8; CONTEXT_DIGEST_BYTES_V1],
    ) -> Result<ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1> {
        let resources = self
            .resources
            .as_ref()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?;
        resources.layout.slot_index_v1(slot)?;
        if resources.layout.context_digest != expected_context_digest {
            return Err(ConfidentialSpoolErrorV1::ContextDigestMismatch);
        }

        let mut resources = self
            .resources
            .take()
            .ok_or(ConfidentialSpoolErrorV1::Poisoned)?;
        let result = read_slot_operation_v1(&mut resources, slot);
        match result {
            Ok(chunk) => {
                self.resources = Some(resources);
                Ok(chunk)
            }
            Err(error) => Err(error),
        }
    }

    /// Consume the snapshot and abort its unlinked backing resources.
    pub fn abort_v1(self) {
        drop(self);
    }
}

struct ConfidentialSpoolResourcesV1 {
    file: ConfidentialSpoolFileV1,
    key: Box<Zeroizing<[u8; KEY_BYTES_V1]>>,
    arena_id: [u8; ARENA_ID_BYTES_V1],
    layout: ConfidentialSpoolLayoutV1,
}

trait EntropySourceV1 {
    fn fill_v1(&mut self, destination: &mut [u8]) -> Result<(), ConfidentialSpoolErrorV1>;
}

struct OsEntropyV1;

impl EntropySourceV1 for OsEntropyV1 {
    fn fill_v1(&mut self, destination: &mut [u8]) -> Result<(), ConfidentialSpoolErrorV1> {
        OsRng
            .try_fill_bytes(destination)
            .map_err(|_| ConfidentialSpoolErrorV1::EntropyUnavailable)
    }
}

struct ConfidentialSpoolFileV1 {
    file: File,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(test)]
    faults: TestFileFaultsV1,
}

impl ConfidentialSpoolFileV1 {
    #[cfg(unix)]
    fn new_v1(file: File, device: u64, inode: u64) -> Self {
        Self {
            file,
            device,
            inode,
            #[cfg(test)]
            faults: TestFileFaultsV1::default(),
        }
    }

    fn validate_detached_v1(&mut self, expected_len: u64) -> Result<(), ConfidentialSpoolErrorV1> {
        self.maybe_fail_v1(TestableFileOperationV1::Metadata)?;
        let metadata = self
            .file
            .metadata()
            .map_err(|error| file_error_v1("metadata", &error))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            if !metadata.file_type().is_file()
                || metadata.permissions().mode() & 0o7777 != 0o600
                || metadata.nlink() != 0
                || metadata.dev() != self.device
                || metadata.ino() != self.inode
            {
                return Err(ConfidentialSpoolErrorV1::UnsafeDetachedFile);
            }
            if metadata.len() != expected_len {
                return Err(ConfidentialSpoolErrorV1::FileLength {
                    expected: expected_len,
                    actual: metadata.len(),
                });
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = (metadata, expected_len);
            Err(ConfidentialSpoolErrorV1::UnsupportedPlatform)
        }
    }

    fn size_v1(&mut self, file_len: u64) -> Result<(), ConfidentialSpoolErrorV1> {
        self.file
            .set_len(file_len)
            .map_err(|error| file_error_v1("set-len", &error))?;
        self.validate_detached_v1(file_len)
    }

    fn write_all_at_v1(
        &mut self,
        bytes: &[u8],
        offset: u64,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        self.maybe_fail_v1(TestableFileOperationV1::Write)?;
        write_all_at_v1(&self.file, bytes, offset)
    }

    fn read_exact_at_v1(
        &mut self,
        bytes: &mut [u8],
        offset: u64,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        self.maybe_fail_v1(TestableFileOperationV1::Read)?;
        read_exact_at_v1(&self.file, bytes, offset)
    }

    #[cfg(not(test))]
    #[expect(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        reason = "production keeps the fallible test fault-injection interface at shared call sites"
    )]
    fn maybe_fail_v1(
        &mut self,
        _operation: TestableFileOperationV1,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        Ok(())
    }

    #[cfg(test)]
    fn maybe_fail_v1(
        &mut self,
        operation: TestableFileOperationV1,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        self.faults.check_v1(operation)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestableFileOperationV1 {
    Metadata,
    Read,
    Write,
}

fn bytes_are_constant_v1(bytes: &[u8]) -> bool {
    bytes
        .first()
        .is_some_and(|first| bytes.iter().all(|byte| byte == first))
}

fn write_slot_operation_v1(
    resources: &mut ConfidentialSpoolResourcesV1,
    slot: u64,
    chunk: &mut ConfidentialSpoolChunkV1,
) -> Result<(), ConfidentialSpoolErrorV1> {
    ensure_safe_detached_file_v1(resources)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&resources.key[..])
        .map_err(|_| ConfidentialSpoolErrorV1::Encryption)?;
    let nonce = nonce_v1(&resources.arena_id, slot);
    let coordinate = derived_coordinate_v1(&resources.layout, slot);
    let aad = aad_v1(&resources.layout, &resources.arena_id, slot, &coordinate)?;
    let tag = cipher
        .encrypt_inout_detached(&nonce, &aad, chunk.as_mut_slice_v1().into())
        .map_err(|_| ConfidentialSpoolErrorV1::Encryption)?;
    let offset = resources.layout.slot_offset_v1(slot)?;
    resources
        .file
        .write_all_at_v1(chunk.as_slice_v1(), offset)?;
    let tag_offset = offset
        .checked_add(resources.layout.plaintext_len)
        .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
    resources.file.write_all_at_v1(tag.as_slice(), tag_offset)?;
    ensure_safe_detached_file_v1(resources)?;
    Ok(())
}

fn seal_operation_v1(
    resources: &mut ConfidentialSpoolResourcesV1,
) -> Result<[u8; 32], ConfidentialSpoolErrorV1> {
    ensure_safe_detached_file_v1(resources)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&resources.key[..])
        .map_err(|_| ConfidentialSpoolErrorV1::Authentication)?;
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(resources.layout.plaintext_len)?;
    let mut tag = aead::Tag::<XChaCha20Poly1305>::default();
    let mut aad = allocate_aad_v1(&resources.layout)?;
    let mut hasher = blake3::Hasher::new();
    hash_snapshot_prefix_v1(&mut hasher, &resources.layout, &resources.arena_id);

    for slot in 0..resources.layout.slot_count {
        let coordinate = derived_coordinate_v1(&resources.layout, slot);
        let offset = resources.layout.slot_offset_v1(slot)?;
        resources
            .file
            .read_exact_at_v1(chunk.as_mut_slice_v1(), offset)?;
        let tag_offset = offset
            .checked_add(resources.layout.plaintext_len)
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
        resources
            .file
            .read_exact_at_v1(tag.as_mut_slice(), tag_offset)?;

        hasher.update(&slot.to_be_bytes());
        hasher.update(&coordinate);
        hasher.update(&resources.layout.ciphertext_record_len.to_be_bytes());
        hasher.update(chunk.as_slice_v1());
        hasher.update(tag.as_slice());

        let nonce = nonce_v1(&resources.arena_id, slot);
        fill_aad_v1(
            &mut aad,
            &resources.layout,
            &resources.arena_id,
            slot,
            &coordinate,
        );
        cipher
            .decrypt_inout_detached(&nonce, &aad, chunk.as_mut_slice_v1().into(), &tag)
            .map_err(|_| ConfidentialSpoolErrorV1::Authentication)?;
        chunk.as_mut_slice_v1().zeroize();
    }
    ensure_safe_detached_file_v1(resources)?;
    Ok(*hasher.finalize().as_bytes())
}

fn read_slot_operation_v1(
    resources: &mut ConfidentialSpoolResourcesV1,
    slot: u64,
) -> Result<ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1> {
    ensure_safe_detached_file_v1(resources)?;
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(resources.layout.plaintext_len)?;
    let mut tag = aead::Tag::<XChaCha20Poly1305>::default();
    let offset = resources.layout.slot_offset_v1(slot)?;
    resources
        .file
        .read_exact_at_v1(chunk.as_mut_slice_v1(), offset)?;
    let tag_offset = offset
        .checked_add(resources.layout.plaintext_len)
        .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
    resources
        .file
        .read_exact_at_v1(tag.as_mut_slice(), tag_offset)?;
    ensure_safe_detached_file_v1(resources)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&resources.key[..])
        .map_err(|_| ConfidentialSpoolErrorV1::Authentication)?;
    let nonce = nonce_v1(&resources.arena_id, slot);
    let coordinate = derived_coordinate_v1(&resources.layout, slot);
    let aad = aad_v1(&resources.layout, &resources.arena_id, slot, &coordinate)?;
    cipher
        .decrypt_inout_detached(&nonce, &aad, chunk.as_mut_slice_v1().into(), &tag)
        .map_err(|_| ConfidentialSpoolErrorV1::Authentication)?;
    Ok(chunk)
}

fn derived_coordinate_v1(
    layout: &ConfidentialSpoolLayoutV1,
    slot: u64,
) -> [u8; CONFIDENTIAL_SPOOL_COORDINATE_BYTES_V1] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COORDINATE_DOMAIN_V1);
    hasher.update(&layout.context_digest);
    hasher.update(&layout.slot_count.to_be_bytes());
    hasher.update(&layout.plaintext_len.to_be_bytes());
    hasher.update(&layout.ciphertext_record_len.to_be_bytes());
    hasher.update(&slot.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn ensure_safe_detached_file_v1(
    resources: &mut ConfidentialSpoolResourcesV1,
) -> Result<(), ConfidentialSpoolErrorV1> {
    resources
        .file
        .validate_detached_v1(resources.layout.file_len)
}

fn nonce_v1(arena_id: &[u8; ARENA_ID_BYTES_V1], slot: u64) -> aead::Nonce<XChaCha20Poly1305> {
    let mut nonce = aead::Nonce::<XChaCha20Poly1305>::default();
    debug_assert_eq!(nonce.len(), NONCE_BYTES_V1);
    nonce[..ARENA_ID_BYTES_V1].copy_from_slice(arena_id);
    nonce[ARENA_ID_BYTES_V1..].copy_from_slice(&slot.to_be_bytes());
    nonce
}

fn aad_v1(
    layout: &ConfidentialSpoolLayoutV1,
    arena_id: &[u8; ARENA_ID_BYTES_V1],
    slot: u64,
    coordinate: &[u8; CONFIDENTIAL_SPOOL_COORDINATE_BYTES_V1],
) -> Result<Zeroizing<Vec<u8>>, ConfidentialSpoolErrorV1> {
    let mut aad = allocate_aad_v1(layout)?;
    fill_aad_v1(&mut aad, layout, arena_id, slot, coordinate);
    Ok(aad)
}

fn allocate_aad_v1(
    layout: &ConfidentialSpoolLayoutV1,
) -> Result<Zeroizing<Vec<u8>>, ConfidentialSpoolErrorV1> {
    let mut aad = Zeroizing::new(Vec::new());
    aad.try_reserve_exact(layout.aad_len)
        .map_err(|_| ConfidentialSpoolErrorV1::Allocation("record AAD"))?;
    Ok(aad)
}

fn fill_aad_v1(
    aad: &mut Zeroizing<Vec<u8>>,
    layout: &ConfidentialSpoolLayoutV1,
    arena_id: &[u8; ARENA_ID_BYTES_V1],
    slot: u64,
    coordinate: &[u8; CONFIDENTIAL_SPOOL_COORDINATE_BYTES_V1],
) {
    aad.clear();
    append_layout_context_v1(aad, layout);
    aad.extend_from_slice(arena_id);
    aad.extend_from_slice(&slot.to_be_bytes());
    aad.extend_from_slice(coordinate);
    aad.extend_from_slice(&layout.plaintext_len.to_be_bytes());
    aad.extend_from_slice(&layout.ciphertext_record_len.to_be_bytes());
    debug_assert_eq!(aad.len(), layout.aad_len);
}

fn append_layout_context_v1(output: &mut Vec<u8>, layout: &ConfidentialSpoolLayoutV1) {
    output.extend_from_slice(AAD_DOMAIN_V1);
    output.extend_from_slice(LAYOUT_DOMAIN_V1);
    output.extend_from_slice(&layout.slot_count.to_be_bytes());
    output.extend_from_slice(&layout.plaintext_len.to_be_bytes());
    output.extend_from_slice(&layout.ciphertext_record_len.to_be_bytes());
    output.extend_from_slice(&layout.file_len.to_be_bytes());
    output.extend_from_slice(&layout.context_digest);
}

fn hash_snapshot_prefix_v1(
    hasher: &mut blake3::Hasher,
    layout: &ConfidentialSpoolLayoutV1,
    arena_id: &[u8; ARENA_ID_BYTES_V1],
) {
    hasher.update(SNAPSHOT_DIGEST_DOMAIN_V1);
    hasher.update(LAYOUT_DOMAIN_V1);
    hasher.update(&layout.slot_count.to_be_bytes());
    hasher.update(&layout.plaintext_len.to_be_bytes());
    hasher.update(&layout.ciphertext_record_len.to_be_bytes());
    hasher.update(&layout.file_len.to_be_bytes());
    hasher.update(&layout.context_digest);
    hasher.update(arena_id);
}

fn file_error_v1(operation: &'static str, error: &io::Error) -> ConfidentialSpoolErrorV1 {
    ConfidentialSpoolErrorV1::FileOperation {
        operation,
        kind: error.kind(),
    }
}

#[cfg(unix)]
fn create_detached_empty_file_v1(
    directory: &Path,
) -> Result<ConfidentialSpoolFileV1, ConfidentialSpoolErrorV1> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let named = tempfile::Builder::new()
        .prefix(".iroha-confidential-spool-v1-")
        .tempfile_in(directory)
        .map_err(|error| file_error_v1("create", &error))?;
    let descriptor_metadata = named
        .as_file()
        .metadata()
        .map_err(|error| file_error_v1("pre-unlink-fstat", &error))?;
    let path_metadata = fs::symlink_metadata(named.path())
        .map_err(|error| file_error_v1("pre-unlink-lstat", &error))?;
    if !safe_pre_unlink_metadata_v1(&descriptor_metadata)
        || !safe_pre_unlink_metadata_v1(&path_metadata)
    {
        return Err(ConfidentialSpoolErrorV1::UnsafeTemporaryFile);
    }
    if descriptor_metadata.dev() != path_metadata.dev()
        || descriptor_metadata.ino() != path_metadata.ino()
    {
        return Err(ConfidentialSpoolErrorV1::TemporaryFileIdentityMismatch);
    }

    let (file, temp_path) = named.into_parts();
    temp_path
        .close()
        .map_err(|error| file_error_v1("unlink", &error))?;
    let detached_metadata = file
        .metadata()
        .map_err(|error| file_error_v1("post-unlink-fstat", &error))?;
    if !detached_metadata.file_type().is_file()
        || detached_metadata.len() != 0
        || detached_metadata.permissions().mode() & 0o7777 != 0o600
        || detached_metadata.nlink() != 0
        || detached_metadata.dev() != descriptor_metadata.dev()
        || detached_metadata.ino() != descriptor_metadata.ino()
    {
        return Err(ConfidentialSpoolErrorV1::UnsafeDetachedFile);
    }

    // TODO: Replace this pathname-oriented tempfile setup with a reviewed
    // dirfd/openat/O_NOFOLLOW construction, then add mlock, MADV_DONTDUMP,
    // fork/descriptor policy, swap/core-dump policy, page-cache, crash,
    // measured RSS, and complete derived-cipher-state zeroization evidence
    // before any release code treats the spool as a hardened lifecycle boundary.
    Ok(ConfidentialSpoolFileV1::new_v1(
        file,
        detached_metadata.dev(),
        detached_metadata.ino(),
    ))
}

#[cfg(unix)]
fn safe_pre_unlink_metadata_v1(metadata: &Metadata) -> bool {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    metadata.file_type().is_file()
        && metadata.len() == 0
        && metadata.permissions().mode() & 0o7777 == 0o600
        && metadata.nlink() == 1
}

#[cfg(not(unix))]
fn create_detached_empty_file_v1(
    _directory: &Path,
) -> Result<ConfidentialSpoolFileV1, ConfidentialSpoolErrorV1> {
    Err(ConfidentialSpoolErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn write_all_at_v1(
    file: &File,
    mut bytes: &[u8],
    mut offset: u64,
) -> Result<(), ConfidentialSpoolErrorV1> {
    use std::os::unix::fs::FileExt as _;

    while !bytes.is_empty() {
        let written = file
            .write_at(bytes, offset)
            .map_err(|error| file_error_v1("write-at", &error))?;
        if written == 0 {
            return Err(ConfidentialSpoolErrorV1::FileOperation {
                operation: "write-at-zero",
                kind: io::ErrorKind::WriteZero,
            });
        }
        bytes = &bytes[written..];
        offset = offset
            .checked_add(
                u64::try_from(written)
                    .map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?,
            )
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn write_all_at_v1(
    _file: &File,
    _bytes: &[u8],
    _offset: u64,
) -> Result<(), ConfidentialSpoolErrorV1> {
    Err(ConfidentialSpoolErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn read_exact_at_v1(
    file: &File,
    mut bytes: &mut [u8],
    mut offset: u64,
) -> Result<(), ConfidentialSpoolErrorV1> {
    use std::os::unix::fs::FileExt as _;

    while !bytes.is_empty() {
        let read = file
            .read_at(bytes, offset)
            .map_err(|error| file_error_v1("read-at", &error))?;
        if read == 0 {
            return Err(ConfidentialSpoolErrorV1::FileOperation {
                operation: "read-at-eof",
                kind: io::ErrorKind::UnexpectedEof,
            });
        }
        let (_, remaining) = bytes.split_at_mut(read);
        bytes = remaining;
        offset = offset
            .checked_add(
                u64::try_from(read).map_err(|_| ConfidentialSpoolErrorV1::AddressSpaceExceeded)?,
            )
            .ok_or(ConfidentialSpoolErrorV1::GeometryOverflow)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn read_exact_at_v1(
    _file: &File,
    _bytes: &mut [u8],
    _offset: u64,
) -> Result<(), ConfidentialSpoolErrorV1> {
    Err(ConfidentialSpoolErrorV1::UnsupportedPlatform)
}

#[cfg(test)]
#[derive(Default)]
struct TestFileFaultsV1 {
    next: Option<TestableFileOperationV1>,
    fail_after: Option<(TestableFileOperationV1, usize)>,
    panic_next: Option<TestableFileOperationV1>,
}

#[cfg(test)]
impl TestFileFaultsV1 {
    fn check_v1(
        &mut self,
        operation: TestableFileOperationV1,
    ) -> Result<(), ConfidentialSpoolErrorV1> {
        if self.panic_next == Some(operation) {
            self.panic_next = None;
            panic!("injected confidential-spool file panic");
        }
        if self.next == Some(operation) {
            self.next = None;
            return Err(ConfidentialSpoolErrorV1::FileOperation {
                operation: "injected-test-failure",
                kind: io::ErrorKind::Other,
            });
        }
        if let Some((target, remaining)) = &mut self.fail_after
            && *target == operation
        {
            if *remaining == 0 {
                self.fail_after = None;
                return Err(ConfidentialSpoolErrorV1::FileOperation {
                    operation: "injected-test-failure",
                    kind: io::ErrorKind::Other,
                });
            }
            *remaining -= 1;
        }
        Ok(())
    }
}

#[cfg(test)]
static TEST_ZEROIZED_CHUNK_DROPS_V1: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::atomic::Ordering,
    };

    use super::*;

    struct ScriptedEntropyV1 {
        key_byte: u8,
        arena_byte: u8,
        calls: Vec<(usize, usize)>,
        fail_call: Option<usize>,
        constant_call: Option<usize>,
    }

    impl ScriptedEntropyV1 {
        fn new_v1(key_byte: u8, arena_byte: u8) -> Self {
            Self {
                key_byte,
                arena_byte,
                calls: Vec::new(),
                fail_call: None,
                constant_call: None,
            }
        }
    }

    impl EntropySourceV1 for ScriptedEntropyV1 {
        fn fill_v1(&mut self, destination: &mut [u8]) -> Result<(), ConfidentialSpoolErrorV1> {
            let call = self.calls.len();
            self.calls
                .push((destination.as_mut_ptr() as usize, destination.len()));
            if self.fail_call == Some(call) {
                return Err(ConfidentialSpoolErrorV1::EntropyUnavailable);
            }
            let byte = match call {
                0 => self.key_byte,
                1 => self.arena_byte,
                _ => return Err(ConfidentialSpoolErrorV1::EntropyUnavailable),
            };
            if self.constant_call == Some(call) {
                destination.fill(byte);
            } else {
                for (index, destination_byte) in destination.iter_mut().enumerate() {
                    *destination_byte = byte.wrapping_add(index.to_le_bytes()[0]);
                }
            }
            Ok(())
        }
    }

    fn layout_v1(slots: u64, plaintext_len: u64, context: &[u8]) -> ConfidentialSpoolLayoutV1 {
        ConfidentialSpoolLayoutV1::new_v1(slots, plaintext_len, context_digest_v1(context))
            .expect("valid test layout")
    }

    fn context_digest_v1(context: &[u8]) -> [u8; CONTEXT_DIGEST_BYTES_V1] {
        *blake3::hash(context).as_bytes()
    }

    fn writer_v1(
        directory: &Path,
        slots: u64,
        plaintext_len: u64,
        context: &[u8],
    ) -> ConfidentialSpoolWriterV1 {
        let mut entropy = ScriptedEntropyV1::new_v1(0x11, 0xA2);
        ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
            directory,
            layout_v1(slots, plaintext_len, context),
            &mut entropy,
        )
        .expect("create test spool")
    }

    fn chunk_v1(bytes: &[u8]) -> ConfidentialSpoolChunkV1 {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(
            u64::try_from(bytes.len()).expect("test length"),
        )
        .expect("allocate test chunk");
        chunk.as_mut_slice_v1().copy_from_slice(bytes);
        chunk
    }

    #[cfg(unix)]
    fn assert_live_descriptor_v1(resources: &ConfidentialSpoolResourcesV1) {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let metadata = resources.file.file.metadata().expect("descriptor metadata");
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 0);
        assert_eq!(metadata.dev(), resources.file.device);
        assert_eq!(metadata.ino(), resources.file.inode);
        assert_eq!(metadata.len(), resources.layout.file_len);
    }

    #[cfg(unix)]
    fn read_record_v1(resources: &mut ConfidentialSpoolResourcesV1, slot: u64) -> Vec<u8> {
        let len =
            usize::try_from(resources.layout.ciphertext_record_len).expect("bounded record length");
        let mut record = vec![0_u8; len];
        let offset = resources
            .layout
            .slot_offset_v1(slot)
            .expect("bounded slot offset");
        resources
            .file
            .read_exact_at_v1(&mut record, offset)
            .expect("read test ciphertext record");
        record
    }

    #[cfg(unix)]
    fn overwrite_record_v1(resources: &mut ConfidentialSpoolResourcesV1, slot: u64, record: &[u8]) {
        assert_eq!(
            u64::try_from(record.len()).expect("test record length"),
            resources.layout.ciphertext_record_len
        );
        let offset = resources
            .layout
            .slot_offset_v1(slot)
            .expect("bounded slot offset");
        resources
            .file
            .write_all_at_v1(record, offset)
            .expect("overwrite test ciphertext record");
    }

    #[test]
    fn release_geometries_are_exact_and_headerless() {
        let q_pcs = layout_v1(194_560, 16_384, b"q-pcs-ten-row-lde-v1");
        assert_eq!(q_pcs.ciphertext_record_len_v1(), 16_400);
        assert_eq!(q_pcs.file_len_v1(), 3_190_784_000);

        let main = layout_v1(466_560, 8_192, b"phase23-external-records-v1");
        assert_eq!(main.ciphertext_record_len_v1(), 8_208);
        assert_eq!(main.file_len_v1(), 3_829_524_480);
        assert_eq!(main.writer_cursor_bytes_v1(), 8);
        let nonce = layout_v1(43, 32, b"phase23-secret-nonces-v1");
        assert_eq!(nonce.file_len_v1(), 2_064);
        assert_eq!(main.file_len_v1() + nonce.file_len_v1(), 3_829_526_544);
    }

    #[test]
    fn geometry_and_context_rejections_precede_external_effects() {
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(0, 1, *blake3::hash(b"context").as_bytes()),
            Err(ConfidentialSpoolErrorV1::EmptyLayout)
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(1, 0, *blake3::hash(b"context").as_bytes()),
            Err(ConfidentialSpoolErrorV1::EmptyChunk)
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(1, 1, [0; CONTEXT_DIGEST_BYTES_V1]),
            Err(ConfidentialSpoolErrorV1::InertContextDigest)
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(
                u64::MAX,
                u64::MAX,
                *blake3::hash(b"context").as_bytes(),
            ),
            Err(ConfidentialSpoolErrorV1::LimitExceeded("slot count"))
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(
                CONFIDENTIAL_SPOOL_MAX_SLOTS_V1 + 1,
                1,
                *blake3::hash(b"context").as_bytes(),
            ),
            Err(ConfidentialSpoolErrorV1::LimitExceeded("slot count"))
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(
                1,
                CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1 + 1,
                *blake3::hash(b"context").as_bytes(),
            ),
            Err(ConfidentialSpoolErrorV1::LimitExceeded(
                "plaintext record length"
            ))
        ));
        assert!(matches!(
            ConfidentialSpoolLayoutV1::new_v1(
                CONFIDENTIAL_SPOOL_MAX_SLOTS_V1,
                8_193,
                *blake3::hash(b"context").as_bytes(),
            ),
            Err(ConfidentialSpoolErrorV1::LimitExceeded("file length"))
        ));
    }

    #[test]
    fn standalone_chunk_constructor_enforces_exact_public_bounds() {
        assert!(matches!(
            ConfidentialSpoolChunkV1::new_zeroed_v1(0),
            Err(ConfidentialSpoolErrorV1::EmptyChunk)
        ));
        let maximum =
            ConfidentialSpoolChunkV1::new_zeroed_v1(CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1)
                .expect("maximum bounded chunk");
        assert_eq!(maximum.len_v1(), CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1);
        assert!(matches!(
            ConfidentialSpoolChunkV1::new_zeroed_v1(CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1 + 1,),
            Err(ConfidentialSpoolErrorV1::LimitExceeded(
                "plaintext chunk length"
            ))
        ));
        assert!(matches!(
            ConfidentialSpoolChunkV1::new_zeroed_v1(u64::MAX),
            Err(ConfidentialSpoolErrorV1::LimitExceeded(
                "plaintext chunk length"
            ))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn entropy_fills_final_heap_key_then_independent_arena() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut entropy = ScriptedEntropyV1::new_v1(0x31, 0xC7);
        let writer = ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
            directory.path(),
            layout_v1(1, 8, b"heap-stable-key-test"),
            &mut entropy,
        )
        .expect("create spool");
        let resources = writer.resources.as_ref().expect("live resources");
        assert_live_descriptor_v1(resources);

        assert_eq!(entropy.calls.len(), 2);
        assert_eq!(entropy.calls[0].1, KEY_BYTES_V1);
        assert_eq!(entropy.calls[1].1, ARENA_ID_BYTES_V1);
        assert_eq!(entropy.calls[0].0, resources.key.as_ptr() as usize);
        assert_eq!(resources.key[0], 0x31);
        assert!(!bytes_are_constant_v1(&resources.key[..]));
        assert_eq!(resources.arena_id[0], 0xC7);
        assert!(!bytes_are_constant_v1(&resources.arena_id));
        assert_ne!(entropy.calls[0].0, entropy.calls[1].0);
    }

    #[test]
    #[cfg(unix)]
    fn entropy_failure_and_inert_material_fail_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut failing = ScriptedEntropyV1::new_v1(0x41, 0x52);
        failing.fail_call = Some(1);
        assert!(matches!(
            ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
                directory.path(),
                layout_v1(1, 8, b"entropy-failure"),
                &mut failing,
            ),
            Err(ConfidentialSpoolErrorV1::EntropyUnavailable)
        ));

        let mut inert_key = ScriptedEntropyV1::new_v1(0, 0x52);
        inert_key.constant_call = Some(0);
        assert!(matches!(
            ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
                directory.path(),
                layout_v1(1, 8, b"inert-key"),
                &mut inert_key,
            ),
            Err(ConfidentialSpoolErrorV1::WeakEntropy("key"))
        ));
        let mut inert_arena = ScriptedEntropyV1::new_v1(0x41, 0);
        inert_arena.constant_call = Some(1);
        assert!(matches!(
            ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
                directory.path(),
                layout_v1(1, 8, b"inert-arena"),
                &mut inert_arena,
            ),
            Err(ConfidentialSpoolErrorV1::WeakEntropy("arena"))
        ));
        let mut aliased = ScriptedEntropyV1::new_v1(0x41, 0x41);
        assert!(matches!(
            ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
                directory.path(),
                layout_v1(1, 8, b"aliased-key-arena"),
                &mut aliased,
            ),
            Err(ConfidentialSpoolErrorV1::WeakEntropy(
                "key/arena separation"
            ))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn public_os_entropy_abort_and_snapshot_geometry_lifecycle() {
        let layout = layout_v1(1, 4, b"public-lifecycle");
        let abort_directory = tempfile::tempdir().expect("abort temporary directory");
        ConfidentialSpoolWriterV1::create_in_v1(abort_directory.path(), layout)
            .expect("create public writer")
            .abort_v1();

        let snapshot_directory = tempfile::tempdir().expect("snapshot temporary directory");
        let mut writer = ConfidentialSpoolWriterV1::create_in_v1(snapshot_directory.path(), layout)
            .expect("create public writer");
        writer
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write slot");
        let snapshot = writer.seal_v1().expect("seal snapshot");
        assert_ne!(snapshot.snapshot_digest_v1(), &[0; 32]);
        assert_eq!(snapshot.slot_count_v1(), 1);
        assert_eq!(snapshot.plaintext_len_v1(), 4);
        assert_eq!(snapshot.ciphertext_record_len_v1(), 20);
        assert_eq!(snapshot.file_len_v1(), 20);
        snapshot.abort_v1();
    }

    #[test]
    fn nonce_and_aad_framing_are_exact() {
        let layout = layout_v1(3, 5, b"ctx");
        let arena = [0xA5; ARENA_ID_BYTES_V1];
        let coordinate = derived_coordinate_v1(&layout, 2);
        let nonce = nonce_v1(&arena, 2);
        assert_eq!(nonce.len(), NONCE_BYTES_V1);
        assert_eq!(&nonce[..ARENA_ID_BYTES_V1], &arena);
        assert_eq!(&nonce[ARENA_ID_BYTES_V1..], &2_u64.to_be_bytes());

        let aad = aad_v1(&layout, &arena, 2, &coordinate).expect("AAD");
        let mut expected = Vec::new();
        expected.extend_from_slice(AAD_DOMAIN_V1);
        expected.extend_from_slice(LAYOUT_DOMAIN_V1);
        expected.extend_from_slice(&3_u64.to_be_bytes());
        expected.extend_from_slice(&5_u64.to_be_bytes());
        expected.extend_from_slice(&21_u64.to_be_bytes());
        expected.extend_from_slice(&63_u64.to_be_bytes());
        expected.extend_from_slice(blake3::hash(b"ctx").as_bytes());
        expected.extend_from_slice(&arena);
        expected.extend_from_slice(&2_u64.to_be_bytes());
        expected.extend_from_slice(&coordinate);
        expected.extend_from_slice(&5_u64.to_be_bytes());
        expected.extend_from_slice(&21_u64.to_be_bytes());
        assert_eq!(aad.as_slice(), expected.as_slice());

        let mut coordinate_frame = Vec::new();
        coordinate_frame.extend_from_slice(COORDINATE_DOMAIN_V1);
        coordinate_frame.extend_from_slice(blake3::hash(b"ctx").as_bytes());
        coordinate_frame.extend_from_slice(&3_u64.to_be_bytes());
        coordinate_frame.extend_from_slice(&5_u64.to_be_bytes());
        coordinate_frame.extend_from_slice(&21_u64.to_be_bytes());
        coordinate_frame.extend_from_slice(&2_u64.to_be_bytes());
        assert_eq!(coordinate, *blake3::hash(&coordinate_frame).as_bytes());
    }

    #[test]
    fn derived_coordinate_separates_context_layout_and_slot() {
        let base = layout_v1(3, 5, b"coordinate-base");
        let other_context = layout_v1(3, 5, b"coordinate-other-context");
        let other_count = layout_v1(4, 5, b"coordinate-base");
        let other_length = layout_v1(3, 6, b"coordinate-base");
        let coordinate = derived_coordinate_v1(&base, 0);

        assert_ne!(coordinate, derived_coordinate_v1(&base, 1));
        assert_ne!(coordinate, derived_coordinate_v1(&other_context, 0));
        assert_ne!(coordinate, derived_coordinate_v1(&other_count, 0));
        assert_ne!(coordinate, derived_coordinate_v1(&other_length, 0));
    }

    #[test]
    #[cfg(unix)]
    fn deterministic_inputs_have_one_canonical_authenticated_digest() {
        let first_dir = tempfile::tempdir().expect("first temporary directory");
        let second_dir = tempfile::tempdir().expect("second temporary directory");
        let mut first = writer_v1(first_dir.path(), 3, 4, b"canonical-order");
        let mut second = writer_v1(second_dir.path(), 3, 4, b"canonical-order");
        for slot in 0_u64..3 {
            let byte = u8::try_from(slot + 1).expect("small slot");
            first
                .write_slot_v1(slot, chunk_v1(&[byte; 4]))
                .expect("write first snapshot");
            second
                .write_slot_v1(slot, chunk_v1(&[byte; 4]))
                .expect("write second snapshot");
        }

        let first = first.seal_v1().expect("seal first");
        let second = second.seal_v1().expect("seal second");
        assert_eq!(first.snapshot_digest_v1(), second.snapshot_digest_v1());
    }

    #[test]
    #[cfg(unix)]
    fn sequential_duplicate_skip_length_and_bounds_preflights_preserve_retry() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 2, 4, b"writer-preflight");
        assert!(matches!(
            writer.write_slot_v1(1, chunk_v1(b"efgh")),
            Err(ConfidentialSpoolErrorV1::UnexpectedWriteSlot {
                expected: 0,
                actual: 1
            })
        ));
        assert!(writer.resources.is_some());
        writer
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("first write");
        assert!(matches!(
            writer.write_slot_v1(0, chunk_v1(b"efgh")),
            Err(ConfidentialSpoolErrorV1::UnexpectedWriteSlot {
                expected: 1,
                actual: 0
            })
        ));
        assert!(matches!(
            writer.write_slot_v1(2, chunk_v1(b"efgh")),
            Err(ConfidentialSpoolErrorV1::SlotOutOfRange { .. })
        ));
        assert!(matches!(
            writer.write_slot_v1(1, chunk_v1(b"bad")),
            Err(ConfidentialSpoolErrorV1::ChunkLength {
                expected: 4,
                actual: 3
            })
        ));
        assert!(writer.resources.is_some());
        writer
            .write_slot_v1(1, chunk_v1(b"efgh"))
            .expect("writer remains usable");
        assert!(matches!(
            writer.write_slot_v1(2, chunk_v1(b"done")),
            Err(ConfidentialSpoolErrorV1::SlotOutOfRange { .. })
        ));
        assert!(writer.resources.is_some());
        writer.seal_v1().expect("complete seal");
    }

    #[test]
    #[cfg(unix)]
    fn roundtrip_and_bounds_preflight_preserve_random_read_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 2, 4, b"roundtrip");
        writer
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write zero");
        writer
            .write_slot_v1(1, chunk_v1(b"wxyz"))
            .expect("write one");
        let mut snapshot = writer.seal_v1().expect("seal");
        assert_live_descriptor_v1(snapshot.resources.as_ref().expect("snapshot resources"));
        assert!(matches!(
            snapshot.read_slot_v1(2, context_digest_v1(b"roundtrip")),
            Err(ConfidentialSpoolErrorV1::SlotOutOfRange { .. })
        ));
        assert!(snapshot.resources.is_some());
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .panic_next = Some(TestableFileOperationV1::Metadata);
        assert!(matches!(
            snapshot.read_slot_v1(1, context_digest_v1(b"wrong-context")),
            Err(ConfidentialSpoolErrorV1::ContextDigestMismatch)
        ));
        assert!(snapshot.resources.is_some());
        assert!(
            snapshot
                .resources
                .as_ref()
                .expect("resources")
                .file
                .faults
                .panic_next
                == Some(TestableFileOperationV1::Metadata)
        );
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .panic_next = None;
        let second = snapshot
            .read_slot_v1(1, context_digest_v1(b"roundtrip"))
            .expect("authenticated read");
        assert_eq!(second.as_slice_v1(), b"wxyz");
        let first = snapshot
            .read_slot_v1(0, context_digest_v1(b"roundtrip"))
            .expect("random authenticated read");
        assert_eq!(first.as_slice_v1(), b"abcd");
    }

    #[test]
    #[cfg(unix)]
    fn injected_write_error_and_unwind_poison_and_zeroize() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 1, 4, b"write-error");
        writer
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .next = Some(TestableFileOperationV1::Write);
        let before = TEST_ZEROIZED_CHUNK_DROPS_V1.load(Ordering::SeqCst);
        assert!(matches!(
            writer.write_slot_v1(0, chunk_v1(b"abcd")),
            Err(ConfidentialSpoolErrorV1::FileOperation { .. })
        ));
        assert!(writer.resources.is_none());
        assert!(TEST_ZEROIZED_CHUNK_DROPS_V1.load(Ordering::SeqCst) > before);

        let tag_dir = tempfile::tempdir().expect("tag-write temporary directory");
        let mut writer = writer_v1(tag_dir.path(), 1, 4, b"tag-write-error");
        writer
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .fail_after = Some((TestableFileOperationV1::Write, 1));
        assert!(matches!(
            writer.write_slot_v1(0, chunk_v1(b"abcd")),
            Err(ConfidentialSpoolErrorV1::FileOperation { .. })
        ));
        assert_eq!(writer.next_slot, 0);
        assert!(writer.resources.is_none());
        assert!(matches!(
            writer.write_slot_v1(0, chunk_v1(b"abcd")),
            Err(ConfidentialSpoolErrorV1::Poisoned)
        ));

        let unwind_dir = tempfile::tempdir().expect("unwind temporary directory");
        let mut writer = writer_v1(unwind_dir.path(), 1, 4, b"write-unwind");
        writer
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .panic_next = Some(TestableFileOperationV1::Metadata);
        let before = TEST_ZEROIZED_CHUNK_DROPS_V1.load(Ordering::SeqCst);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = writer.write_slot_v1(0, chunk_v1(b"abcd"));
        }));
        assert!(unwind.is_err());
        assert!(writer.resources.is_none());
        assert!(TEST_ZEROIZED_CHUNK_DROPS_V1.load(Ordering::SeqCst) > before);
    }

    #[test]
    #[cfg(unix)]
    fn seal_authenticates_tampering_and_requires_every_slot() {
        let incomplete_dir = tempfile::tempdir().expect("incomplete temporary directory");
        let mut incomplete = writer_v1(incomplete_dir.path(), 2, 4, b"incomplete");
        incomplete
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write one slot");
        assert!(matches!(
            incomplete.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Incomplete { remaining: 1 })
        ));

        let tamper_dir = tempfile::tempdir().expect("tamper temporary directory");
        let mut tampered = writer_v1(tamper_dir.path(), 1, 4, b"tamper");
        tampered
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write slot");
        let resources = tampered.resources.as_mut().expect("resources");
        let mut byte = [0_u8; 1];
        resources
            .file
            .read_exact_at_v1(&mut byte, 0)
            .expect("read ciphertext byte");
        byte[0] ^= 1;
        resources
            .file
            .write_all_at_v1(&byte, 0)
            .expect("tamper ciphertext byte");
        assert!(matches!(
            tampered.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Authentication)
        ));

        let tag_dir = tempfile::tempdir().expect("tag-tamper temporary directory");
        let mut tampered = writer_v1(tag_dir.path(), 1, 4, b"tag-tamper");
        tampered
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write tag fixture");
        let resources = tampered.resources.as_mut().expect("resources");
        let tag_offset = resources.layout.plaintext_len;
        let mut byte = [0_u8; 1];
        resources
            .file
            .read_exact_at_v1(&mut byte, tag_offset)
            .expect("read tag byte");
        byte[0] ^= 1;
        resources
            .file
            .write_all_at_v1(&byte, tag_offset)
            .expect("tamper tag byte");
        assert!(matches!(
            tampered.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Authentication)
        ));
    }

    #[test]
    #[cfg(unix)]
    fn seal_rejects_record_duplicate_swap_and_cross_arena_replay() {
        let duplicate_dir = tempfile::tempdir().expect("duplicate temporary directory");
        let mut duplicate = writer_v1(duplicate_dir.path(), 2, 4, b"duplicate-record");
        duplicate
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write duplicate zero");
        duplicate
            .write_slot_v1(1, chunk_v1(b"wxyz"))
            .expect("write duplicate one");
        let resources = duplicate.resources.as_mut().expect("resources");
        let first = read_record_v1(resources, 0);
        overwrite_record_v1(resources, 1, &first);
        assert!(matches!(
            duplicate.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Authentication)
        ));

        let swap_dir = tempfile::tempdir().expect("swap temporary directory");
        let mut swapped = writer_v1(swap_dir.path(), 2, 4, b"swap-records");
        swapped
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write swap zero");
        swapped
            .write_slot_v1(1, chunk_v1(b"wxyz"))
            .expect("write swap one");
        let resources = swapped.resources.as_mut().expect("resources");
        let first = read_record_v1(resources, 0);
        let second = read_record_v1(resources, 1);
        overwrite_record_v1(resources, 0, &second);
        overwrite_record_v1(resources, 1, &first);
        assert!(matches!(
            swapped.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Authentication)
        ));

        let source_dir = tempfile::tempdir().expect("source-arena temporary directory");
        let target_dir = tempfile::tempdir().expect("target-arena temporary directory");
        let layout = layout_v1(1, 4, b"cross-arena");
        let mut source_entropy = ScriptedEntropyV1::new_v1(0x11, 0xA2);
        let mut target_entropy = ScriptedEntropyV1::new_v1(0x11, 0xB3);
        let mut source = ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
            source_dir.path(),
            layout,
            &mut source_entropy,
        )
        .expect("create source arena");
        let mut target = ConfidentialSpoolWriterV1::create_in_with_entropy_v1(
            target_dir.path(),
            layout,
            &mut target_entropy,
        )
        .expect("create target arena");
        source
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write source arena");
        target
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write target arena");
        let replay = read_record_v1(source.resources.as_mut().expect("source resources"), 0);
        overwrite_record_v1(
            target.resources.as_mut().expect("target resources"),
            0,
            &replay,
        );
        assert!(matches!(
            target.seal_v1(),
            Err(ConfidentialSpoolErrorV1::Authentication)
        ));
    }

    #[test]
    #[cfg(unix)]
    fn seal_rejects_key_arena_context_and_file_geometry_substitution() {
        for substitution in ["key", "arena", "context"] {
            let directory = tempfile::tempdir().expect("substitution temporary directory");
            let mut writer = writer_v1(directory.path(), 1, 4, b"substitution");
            writer
                .write_slot_v1(0, chunk_v1(b"abcd"))
                .expect("write substitution fixture");
            let resources = writer.resources.as_mut().expect("resources");
            match substitution {
                "key" => resources.key[0] ^= 1,
                "arena" => resources.arena_id[0] ^= 1,
                "context" => resources.layout.context_digest[0] ^= 1,
                _ => unreachable!("fixed substitutions"),
            }
            assert!(matches!(
                writer.seal_v1(),
                Err(ConfidentialSpoolErrorV1::Authentication)
            ));
        }

        for changed_len in [19_u64, 21] {
            let directory = tempfile::tempdir().expect("geometry temporary directory");
            let mut writer = writer_v1(directory.path(), 1, 4, b"geometry-substitution");
            writer
                .write_slot_v1(0, chunk_v1(b"abcd"))
                .expect("write geometry fixture");
            writer
                .resources
                .as_mut()
                .expect("resources")
                .file
                .file
                .set_len(changed_len)
                .expect("change test file length");
            assert!(matches!(
                writer.seal_v1(),
                Err(ConfidentialSpoolErrorV1::FileLength { .. })
            ));
        }
    }

    #[test]
    #[cfg(unix)]
    fn every_operational_read_error_poison_drops_resources() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 1, 4, b"read-failure");
        writer.write_slot_v1(0, chunk_v1(b"abcd")).expect("write");
        let mut snapshot = writer.seal_v1().expect("seal");
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .next = Some(TestableFileOperationV1::Read);
        assert!(matches!(
            snapshot.read_slot_v1(0, context_digest_v1(b"read-failure")),
            Err(ConfidentialSpoolErrorV1::FileOperation { .. })
        ));
        assert!(snapshot.resources.is_none());
        assert!(matches!(
            snapshot.read_slot_v1(0, context_digest_v1(b"read-failure")),
            Err(ConfidentialSpoolErrorV1::Poisoned)
        ));

        let unwind_dir = tempfile::tempdir().expect("read-unwind temporary directory");
        let mut writer = writer_v1(unwind_dir.path(), 1, 4, b"read-unwind");
        writer
            .write_slot_v1(0, chunk_v1(b"abcd"))
            .expect("write unwind fixture");
        let mut snapshot = writer.seal_v1().expect("seal unwind fixture");
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .faults
            .panic_next = Some(TestableFileOperationV1::Read);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = snapshot.read_slot_v1(0, context_digest_v1(b"read-unwind"));
        }));
        assert!(unwind.is_err());
        assert!(snapshot.resources.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn each_read_revalidates_exact_file_length() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 1, 4, b"length-recheck");
        writer.write_slot_v1(0, chunk_v1(b"abcd")).expect("write");
        let mut snapshot = writer.seal_v1().expect("seal");
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .file
            .set_len(19)
            .expect("truncate test file");
        assert!(matches!(
            snapshot.read_slot_v1(0, context_digest_v1(b"length-recheck")),
            Err(ConfidentialSpoolErrorV1::FileLength {
                expected: 20,
                actual: 19
            })
        ));
        assert!(snapshot.resources.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn each_read_revalidates_private_descriptor_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("temporary directory");
        let mut writer = writer_v1(directory.path(), 1, 4, b"mode-recheck");
        writer.write_slot_v1(0, chunk_v1(b"abcd")).expect("write");
        let mut snapshot = writer.seal_v1().expect("seal");
        snapshot
            .resources
            .as_mut()
            .expect("resources")
            .file
            .file
            .set_permissions(std::fs::Permissions::from_mode(0o640))
            .expect("change test mode");
        assert!(matches!(
            snapshot.read_slot_v1(0, context_digest_v1(b"mode-recheck")),
            Err(ConfidentialSpoolErrorV1::UnsafeDetachedFile)
        ));
        assert!(snapshot.resources.is_none());
    }

    #[test]
    #[expect(clippy::too_many_lines, reason = "cohesive ownership source contract")]
    fn public_owners_have_no_clone_debug_or_escape_surface_in_source() {
        const TEST_MODULE_MARKER: &str = "#[cfg(test)]\nmod tests {";
        fn public_method_names_v1(region: &str) -> Vec<&str> {
            region
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    line.strip_prefix("pub fn ")
                        .or_else(|| line.strip_prefix("pub const fn "))
                        .and_then(|suffix| suffix.split_once('(').map(|(name, _)| name))
                })
                .collect()
        }

        let source = include_str!("confidential_spool.rs");
        assert_eq!(source.matches(TEST_MODULE_MARKER).count(), 1);
        let production = source
            .split_once(TEST_MODULE_MARKER)
            .expect("inline test module marker")
            .0;

        let chunk_start = production
            .find("pub struct ConfidentialSpoolChunkV1")
            .expect("chunk owner declaration");
        let writer_start = production
            .find("pub struct ConfidentialSpoolWriterV1")
            .expect("writer owner declaration");
        let snapshot_start = production
            .find("pub struct ConfidentialSpoolSnapshotV1")
            .expect("snapshot owner declaration");
        let resources_start = production
            .find("struct ConfidentialSpoolResourcesV1")
            .expect("private resources declaration");
        assert_eq!(
            public_method_names_v1(&production[chunk_start..writer_start]),
            ["new_zeroed_v1", "as_slice_v1", "as_mut_slice_v1", "len_v1"]
        );
        assert_eq!(
            public_method_names_v1(&production[writer_start..snapshot_start]),
            ["create_in_v1", "write_slot_v1", "seal_v1", "abort_v1"]
        );
        assert_eq!(
            public_method_names_v1(&production[snapshot_start..resources_start]),
            [
                "snapshot_digest_v1",
                "slot_count_v1",
                "plaintext_len_v1",
                "ciphertext_record_len_v1",
                "file_len_v1",
                "read_slot_v1",
                "abort_v1",
            ]
        );
        for owner in [
            "ConfidentialSpoolWriterV1",
            "ConfidentialSpoolSnapshotV1",
            "ConfidentialSpoolChunkV1",
        ] {
            assert!(!production.contains(&format!("impl Clone for {owner}")));
            assert!(!production.contains(&format!("impl Debug for {owner}")));
            let declaration = production
                .find(&format!("pub struct {owner}"))
                .expect("public owner declaration");
            let preceding = &production[declaration.saturating_sub(240)..declaration];
            assert!(!preceding.contains("#[derive"));
            for forbidden_trait in [
                "Clone",
                "Debug",
                "Deref",
                "DerefMut",
                "AsRef",
                "AsMut",
                "From",
                "Into",
                "Encode",
                "Decode",
                "Serialize",
                "Deserialize",
                "IntoSchema",
            ] {
                assert!(
                    !production.contains(&format!("impl {forbidden_trait} for {owner}")),
                    "forbidden owner trait {forbidden_trait} on {owner}"
                );
            }
        }
        for forbidden in [
            concat!("fn path_", "v1("),
            concat!("fn key_", "v1("),
            concat!("fn file_", "v1("),
            concat!("fn reopen_", "v1("),
            concat!("fn into_vec_", "v1("),
            concat!("fn take_", "v1("),
            concat!("fn from_slice_", "v1("),
        ] {
            assert!(!production.contains(forbidden));
        }
        for forbidden_state in [
            "coordinates:",
            "coordinate_table",
            "FilledSlotBitset",
            "filled-slot bitset",
            "HashMap",
        ] {
            assert!(!production.contains(forbidden_state));
        }
        assert!(production.contains("Option<ConfidentialSpoolResourcesV1>"));
        assert!(production.contains(".resources\n            .take()"));
        assert!(production.contains("key: Box<Zeroizing<[u8; KEY_BYTES_V1]>>"));
        assert!(production.contains("bytes: Zeroizing<Box<[u8]>>"));
        assert!(production.contains("temp_path"));
        assert!(production.contains(".close()"));
        assert!(!production.contains("fs::remove_file"));
        assert!(production.contains("metadata.nlink() != 0"));
    }
}
