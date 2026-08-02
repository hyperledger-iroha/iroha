//! Content-addressed object transport for direct-relation verification.
//!
//! A sound direct-relation verifier needs the actual canonical public RNS
//! polynomials, not only digests copied into a statement.  Those polynomials
//! are too large to inline in a proof envelope, so this module establishes the
//! phase-one boundary for reading them from an immutable object provider.
//!
//! The provider's identity and snapshot label are treated only as freshness
//! signals.  They are checked before and after every bounded `read_at`, but a
//! malicious provider may keep both labels stable while changing bytes.  The
//! streaming read transaction therefore hashes every byte and issues a receipt
//! only after the complete BLAKE3 content address matches.  A caller must not
//! mint a proof/admission capability until that transaction has finished.
//!
//! This module intentionally closes no proof, admission, or release gate.

#![allow(dead_code)]

use super::{MKHE_VERSION_V1, ZkAmsMkheErrorV1};
use crate::vega::sponge::Keccak256;

const DIRECT_OBJECT_POINTER_TAG_V1: [u8; 4] = *b"ZDOP";
const DIRECT_OBJECT_POINTER_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-pointer";
const DIRECT_OBJECT_SNAPSHOT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-snapshot";
const DIRECT_OBJECT_READ_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-read-receipt";

/// Fixed width of one canonical direct-object pointer frame.
pub(super) const ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1: usize = 4 + 1 + 1 + 8 + 32 + 32;
/// Sole maximum request passed to an untrusted direct-object provider.
pub(super) const ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1: usize = 8 * 1024;
/// Existing first-release ceiling for one canonical RNS polynomial object.
const DIRECT_RNS_POLYNOMIAL_MAX_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Existing first-release ceiling for one standalone native proof object.
const DIRECT_RELATION_PROOF_MAX_BYTES_V1: u64 = 32 * 1024 * 1024;

/// Phase one is transport plumbing only; it is not direct-proof admission.
pub(super) const ZK_AMS_MKHE_DIRECT_OBJECT_ADMISSION_GATE_V1: bool = false;
/// No release gate may depend on this module until the relation verifier lands.
pub(super) const ZK_AMS_MKHE_DIRECT_OBJECT_RELEASE_GATE_V1: bool = false;

/// Canonical type of a separately addressed direct-relation object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum ZkAmsMkheDirectObjectKindV1 {
    /// Party-local first-round `H0_i` polynomial.
    RkgH0 = 1,
    /// Party-local first-round `H1_i` polynomial.
    RkgH1 = 2,
    /// Party-local second-round `K_i` polynomial.
    RkgK = 3,
    /// Party-local public-`A` normalization polynomial.
    RkgNormalization = 4,
    /// Party-local Galois-key constant-component polynomial.
    GaloisB = 5,
    /// Coordinator-derived aggregate first-round `H0` polynomial.
    AggregateH0 = 6,
    /// Coordinator-derived aggregate first-round `H1` polynomial.
    AggregateH1 = 7,
    /// One exact canonical native direct-relation proof envelope.
    ProofEnvelope = 8,
    /// Party-local collective-public-key `b_i` polynomial.
    CpkPartyB = 9,
    /// Exact native proof linking CPK membership commitments to `b_i`.
    CpkRelationProof = 10,
}

impl ZkAmsMkheDirectObjectKindV1 {
    const fn payload_ceiling(self) -> u64 {
        match self {
            Self::RkgH0
            | Self::RkgH1
            | Self::RkgK
            | Self::RkgNormalization
            | Self::GaloisB
            | Self::AggregateH0
            | Self::AggregateH1
            | Self::CpkPartyB => DIRECT_RNS_POLYNOMIAL_MAX_BYTES_V1,
            Self::ProofEnvelope | Self::CpkRelationProof => DIRECT_RELATION_PROOF_MAX_BYTES_V1,
        }
    }
}

impl TryFrom<u8> for ZkAmsMkheDirectObjectKindV1 {
    type Error = ZkAmsMkheErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::RkgH0),
            2 => Ok(Self::RkgH1),
            3 => Ok(Self::RkgK),
            4 => Ok(Self::RkgNormalization),
            5 => Ok(Self::GaloisB),
            6 => Ok(Self::AggregateH0),
            7 => Ok(Self::AggregateH1),
            8 => Ok(Self::ProofEnvelope),
            9 => Ok(Self::CpkPartyB),
            10 => Ok(Self::CpkRelationProof),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// Exact typed BLAKE3 content address of one direct-relation object.
///
/// The trailing Keccak digest authenticates the canonical pointer frame and
/// prevents any caller from treating length, kind, or content hash as an
/// unbound side channel.  It does not replace validation of the addressed
/// payload's BLAKE3 hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectObjectPointerV1 {
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
    pointer_digest: [u8; 32],
}

impl ZkAmsMkheDirectObjectPointerV1 {
    /// Construct a canonical pointer from an independently computed content address.
    pub(super) fn new(
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
        payload_blake3: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if payload_bytes == 0 || payload_blake3 == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if payload_bytes > kind.payload_ceiling() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let mut value = Self {
            kind,
            payload_bytes,
            payload_blake3,
            pointer_digest: [0; 32],
        };
        value.pointer_digest = direct_object_pointer_digest(value);
        value.validate_for_kind(kind)?;
        Ok(value)
    }

    /// Construct a pointer by hashing one already bounded in-memory payload.
    pub(super) fn from_payload(
        kind: ZkAmsMkheDirectObjectKindV1,
        payload: &[u8],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let payload_bytes =
            u64::try_from(payload.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if payload_bytes > kind.payload_ceiling() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Self::new(kind, payload_bytes, norito::streaming::blake3_hash(payload))
    }

    fn validate_for_kind(
        self,
        expected_kind: ZkAmsMkheDirectObjectKindV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.kind != expected_kind
            || self.payload_bytes == 0
            || self.payload_bytes > self.kind.payload_ceiling()
            || self.payload_blake3 == [0; 32]
            || self.pointer_digest == [0; 32]
            || self.pointer_digest != direct_object_pointer_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    /// Encode the sole fixed-width, big-endian pointer frame.
    #[must_use]
    pub(super) fn encode(self) -> [u8; ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1] {
        let mut bytes = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1];
        bytes[..4].copy_from_slice(&DIRECT_OBJECT_POINTER_TAG_V1);
        bytes[4] = MKHE_VERSION_V1;
        bytes[5] = self.kind as u8;
        bytes[6..14].copy_from_slice(&self.payload_bytes.to_be_bytes());
        bytes[14..46].copy_from_slice(&self.payload_blake3);
        bytes[46..78].copy_from_slice(&self.pointer_digest);
        bytes
    }

    /// Decode exactly one pointer and reject truncation, trailing bytes, and kind confusion.
    pub(super) fn decode_exact(
        expected_kind: ZkAmsMkheDirectObjectKindV1,
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1
            || bytes[..4] != DIRECT_OBJECT_POINTER_TAG_V1
            || bytes[4] != MKHE_VERSION_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let kind = ZkAmsMkheDirectObjectKindV1::try_from(bytes[5])?;
        if kind != expected_kind {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut payload_bytes = [0_u8; 8];
        payload_bytes.copy_from_slice(&bytes[6..14]);
        let mut payload_blake3 = [0_u8; 32];
        payload_blake3.copy_from_slice(&bytes[14..46]);
        let mut pointer_digest = [0_u8; 32];
        pointer_digest.copy_from_slice(&bytes[46..78]);
        let value = Self {
            kind,
            payload_bytes: u64::from_be_bytes(payload_bytes),
            payload_blake3,
            pointer_digest,
        };
        value.validate_for_kind(expected_kind)?;
        Ok(value)
    }

    /// Bound object kind.
    #[must_use]
    pub(super) const fn kind(self) -> ZkAmsMkheDirectObjectKindV1 {
        self.kind
    }

    /// Exact complete object length.
    #[must_use]
    pub(super) const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }

    /// BLAKE3 digest of every byte in the exact complete object.
    #[must_use]
    pub(super) const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }

    /// Domain-separated digest of the complete canonical pointer frame.
    #[must_use]
    pub(super) const fn pointer_digest(self) -> [u8; 32] {
        self.pointer_digest
    }
}

fn direct_object_pointer_digest(pointer: ZkAmsMkheDirectObjectPointerV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_POINTER_DOMAIN_V1);
    hash.update(&DIRECT_OBJECT_POINTER_TAG_V1);
    hash.update(&[MKHE_VERSION_V1, pointer.kind as u8]);
    hash.update(&pointer.payload_bytes.to_be_bytes());
    hash.update(&pointer.payload_blake3);
    hash.finalize()
}

/// Stable random-access view of a set of content-addressed direct objects.
///
/// `provider_identity` names this exact open provider session.
/// `snapshot_identity` names the immutable revision visible to the session and
/// must not encode object pointers, request offsets, or call counts.  Every
/// object operation carries its exact pointer, so one provider snapshot can
/// serve all public polynomials and the proof without mutable object selection.
/// `read_at` performs one absolute, non-retrying read; a short result is always
/// rejected by the canonical adapter.
pub(super) trait ZkAmsMkheDirectObjectReadAtProviderV1 {
    /// Nonzero identity of this exact open provider session.
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1>;

    /// Nonzero immutable content revision visible through this session.
    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1>;

    /// Exact complete logical length of the object at `pointer`.
    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1>;

    /// Read once at one checked absolute object offset.
    ///
    /// Implementations return the number of bytes initialized in
    /// `destination`.  Returning anything other than `destination.len()` is a
    /// hard failure; the transport never repairs or retries a short read.
    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1>;
}

/// Exact provider session and immutable snapshot captured for one object pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectObjectSnapshotV1 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    snapshot_binding_digest: [u8; 32],
}

impl ZkAmsMkheDirectObjectSnapshotV1 {
    fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        self.pointer.validate_for_kind(self.pointer.kind)?;
        if self.provider_identity == [0; 32]
            || self.snapshot_identity == [0; 32]
            || self.snapshot_binding_digest == [0; 32]
            || self.snapshot_binding_digest != direct_object_snapshot_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Exact typed content address bound to this pass.
    #[must_use]
    pub(super) const fn pointer(self) -> ZkAmsMkheDirectObjectPointerV1 {
        self.pointer
    }

    /// Exact open-provider session identity bound to this pass.
    #[must_use]
    pub(super) const fn provider_identity(self) -> [u8; 32] {
        self.provider_identity
    }

    /// Exact immutable revision identity bound to this pass.
    #[must_use]
    pub(super) const fn snapshot_identity(self) -> [u8; 32] {
        self.snapshot_identity
    }

    /// Digest binding the provider session, snapshot, and canonical pointer.
    #[must_use]
    pub(super) const fn snapshot_binding_digest(self) -> [u8; 32] {
        self.snapshot_binding_digest
    }
}

fn direct_object_snapshot_digest(snapshot: ZkAmsMkheDirectObjectSnapshotV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_SNAPSHOT_DOMAIN_V1);
    hash.update(&snapshot.pointer.pointer_digest);
    hash.update(&snapshot.provider_identity);
    hash.update(&snapshot.snapshot_identity);
    hash.finalize()
}

/// Bind one provider session to one exact object before any payload read.
pub(super) fn bind_zk_ams_mkhe_direct_object_snapshot_v1<P>(
    expected_kind: ZkAmsMkheDirectObjectKindV1,
    expected_pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
) -> Result<ZkAmsMkheDirectObjectSnapshotV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    expected_pointer.validate_for_kind(expected_kind)?;
    let provider_identity = provider.provider_identity()?;
    let snapshot_identity = provider.snapshot_identity()?;
    let payload_len = provider.object_len(expected_pointer)?;
    if provider_identity == [0; 32]
        || snapshot_identity == [0; 32]
        || payload_len != expected_pointer.payload_bytes
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut snapshot = ZkAmsMkheDirectObjectSnapshotV1 {
        pointer: expected_pointer,
        provider_identity,
        snapshot_identity,
        snapshot_binding_digest: [0; 32],
    };
    snapshot.snapshot_binding_digest = direct_object_snapshot_digest(snapshot);
    snapshot.validate()?;
    Ok(snapshot)
}

/// Re-observe every provider axis and reject any drift from a bound snapshot.
pub(super) fn ensure_zk_ams_mkhe_direct_object_snapshot_v1<P>(
    expected: ZkAmsMkheDirectObjectSnapshotV1,
    provider: &mut P,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    expected.validate()?;
    let observed = bind_zk_ams_mkhe_direct_object_snapshot_v1(
        expected.pointer.kind,
        expected.pointer,
        provider,
    )?;
    if observed != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

/// Perform one allocation-bounded exact absolute read under a bound snapshot.
///
/// This helper checks provider state but does not authenticate an isolated
/// subrange.  Sound consumers use it through
/// [`ZkAmsMkheDirectObjectReadTransactionV1`], whose final receipt binds the
/// hash of the complete canonical object.
pub(super) fn read_zk_ams_mkhe_direct_object_at_exact_v1<P>(
    snapshot: ZkAmsMkheDirectObjectSnapshotV1,
    provider: &mut P,
    absolute_offset: u64,
    destination: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    snapshot.validate()?;
    if destination.is_empty() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if destination.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let requested =
        u64::try_from(destination.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = absolute_offset
        .checked_add(requested)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if end > snapshot.pointer.payload_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    ensure_zk_ams_mkhe_direct_object_snapshot_v1(snapshot, provider)?;
    let read = provider.read_at(snapshot.pointer, absolute_offset, destination)?;
    if read != destination.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    ensure_zk_ams_mkhe_direct_object_snapshot_v1(snapshot, provider)
}

/// Receipt for the exact bytes consumed in one complete snapshot-bound pass.
///
/// The receipt certifies only that completed pass.  It must never authorize a
/// later read without another complete content-hash validation.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectObjectReadReceiptV1 {
    snapshot: ZkAmsMkheDirectObjectSnapshotV1,
    canonical_bytes: u64,
    payload_blake3: [u8; 32],
    receipt_digest: [u8; 32],
}

impl ZkAmsMkheDirectObjectReadReceiptV1 {
    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.snapshot.validate()?;
        if self.canonical_bytes != self.snapshot.pointer.payload_bytes
            || self.payload_blake3 != self.snapshot.pointer.payload_blake3
            || self.payload_blake3 == [0; 32]
            || self.receipt_digest == [0; 32]
            || self.receipt_digest != direct_object_read_receipt_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    /// Exact provider snapshot whose bytes were consumed.
    #[must_use]
    pub(super) const fn snapshot(&self) -> ZkAmsMkheDirectObjectSnapshotV1 {
        self.snapshot
    }

    /// Exact number of canonical bytes consumed.
    #[must_use]
    pub(super) const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }

    /// Independently recomputed BLAKE3 digest of the complete byte stream.
    #[must_use]
    pub(super) const fn payload_blake3(&self) -> [u8; 32] {
        self.payload_blake3
    }

    /// Digest binding all receipt axes.
    #[must_use]
    pub(super) const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
}

fn direct_object_read_receipt_digest(receipt: &ZkAmsMkheDirectObjectReadReceiptV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_READ_RECEIPT_DOMAIN_V1);
    hash.update(&receipt.snapshot.snapshot_binding_digest);
    hash.update(&receipt.snapshot.pointer.pointer_digest);
    hash.update(&receipt.canonical_bytes.to_be_bytes());
    hash.update(&receipt.payload_blake3);
    hash.finalize()
}

/// Single-use sequential read transaction over one random-access provider.
///
/// Any invalid request or provider failure permanently poisons the transaction;
/// callers cannot retry into a different byte stream under the same hash state.
pub(super) struct ZkAmsMkheDirectObjectReadTransactionV1 {
    snapshot: ZkAmsMkheDirectObjectSnapshotV1,
    next_offset: u64,
    payload_hasher: norito::streaming::Blake3Hasher,
    failed: bool,
}

impl core::fmt::Debug for ZkAmsMkheDirectObjectReadTransactionV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectReadTransactionV1")
            .field("pointer", &self.snapshot.pointer)
            .field("next_offset", &self.next_offset)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectObjectReadTransactionV1 {
    /// Begin one snapshot-bound pass before touching object bytes.
    pub(super) fn begin<P>(
        expected_kind: ZkAmsMkheDirectObjectKindV1,
        expected_pointer: ZkAmsMkheDirectObjectPointerV1,
        provider: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let snapshot =
            bind_zk_ams_mkhe_direct_object_snapshot_v1(expected_kind, expected_pointer, provider)?;
        Ok(Self {
            snapshot,
            next_offset: 0,
            payload_hasher: norito::streaming::Blake3Hasher::new(),
            failed: false,
        })
    }

    /// Read and hash the next canonical chunk.
    ///
    /// A zero return denotes clean logical EOF and performs no provider call.
    pub(super) fn read_next<P>(
        &mut self,
        provider: &mut P,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        // Poison before entering provider-controlled code.  If a hostile
        // provider panics and an outer boundary catches the unwind, this pass
        // must remain unusable instead of resuming with partially observed
        // provider state or hash input.
        self.failed = true;
        let result = self.read_next_inner(provider, destination);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn read_next_inner<P>(
        &mut self,
        provider: &mut P,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.next_offset == self.snapshot.pointer.payload_bytes {
            return Ok(0);
        }
        if destination.is_empty() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if destination.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let remaining = self
            .snapshot
            .pointer
            .payload_bytes
            .checked_sub(self.next_offset)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let take = destination.len().min(
            usize::try_from(remaining).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        );
        read_zk_ams_mkhe_direct_object_at_exact_v1(
            self.snapshot,
            provider,
            self.next_offset,
            &mut destination[..take],
        )?;
        self.payload_hasher.update(&destination[..take]);
        self.next_offset = self
            .next_offset
            .checked_add(
                u64::try_from(take).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        Ok(take)
    }

    /// Authenticate the exact complete stream and issue the sole read receipt.
    pub(super) fn finish<P>(
        self,
        provider: &mut P,
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.failed || self.next_offset != self.snapshot.pointer.payload_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        ensure_zk_ams_mkhe_direct_object_snapshot_v1(self.snapshot, provider)?;
        let payload_blake3 = self.payload_hasher.finalize();
        if payload_blake3 != self.snapshot.pointer.payload_blake3 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut receipt = ZkAmsMkheDirectObjectReadReceiptV1 {
            snapshot: self.snapshot,
            canonical_bytes: self.next_offset,
            payload_blake3,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = direct_object_read_receipt_digest(&receipt);
        receipt.validate()?;
        Ok(receipt)
    }

    /// Bytes still required before `finish` can succeed.
    #[must_use]
    pub(super) const fn remaining_bytes(&self) -> u64 {
        self.snapshot.pointer.payload_bytes - self.next_offset
    }
}

/// Validate one complete object with fixed workspace and no payload allocation.
pub(super) fn validate_zk_ams_mkhe_direct_object_v1<P>(
    expected_kind: ZkAmsMkheDirectObjectKindV1,
    expected_pointer: ZkAmsMkheDirectObjectPointerV1,
    provider: &mut P,
) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let mut transaction =
        ZkAmsMkheDirectObjectReadTransactionV1::begin(expected_kind, expected_pointer, provider)?;
    let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    while transaction.remaining_bytes() != 0 {
        let read = transaction.read_next(provider, &mut buffer)?;
        if read == 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
    }
    transaction.finish(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVIDER_ID: [u8; 32] = [0x51; 32];
    const DRIFTED_PROVIDER_ID: [u8; 32] = [0x52; 32];
    const SNAPSHOT_ID: [u8; 32] = [0x61; 32];
    const DRIFTED_SNAPSHOT_ID: [u8; 32] = [0x62; 32];

    #[derive(Clone)]
    struct TestProvider {
        objects: Vec<(ZkAmsMkheDirectObjectPointerV1, Vec<u8>)>,
        provider_identity: [u8; 32],
        snapshot_identity: [u8; 32],
        identity_calls: usize,
        snapshot_calls: usize,
        len_calls: usize,
        read_calls: usize,
        identity_drift_at: Option<usize>,
        snapshot_drift_at: Option<usize>,
        len_drift_at: Option<usize>,
        short_read_at: Option<usize>,
        over_read_at: Option<usize>,
        panic_read_at: Option<usize>,
        mutate_same_snapshot_at: Option<usize>,
        substitute_read_at: Option<usize>,
        substitute_read_with: Option<ZkAmsMkheDirectObjectPointerV1>,
        len_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
        read_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    }

    impl TestProvider {
        fn new(kind: ZkAmsMkheDirectObjectKindV1, bytes: Vec<u8>) -> Self {
            let pointer = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();
            Self {
                objects: vec![(pointer, bytes)],
                provider_identity: PROVIDER_ID,
                snapshot_identity: SNAPSHOT_ID,
                identity_calls: 0,
                snapshot_calls: 0,
                len_calls: 0,
                read_calls: 0,
                identity_drift_at: None,
                snapshot_drift_at: None,
                len_drift_at: None,
                short_read_at: None,
                over_read_at: None,
                panic_read_at: None,
                mutate_same_snapshot_at: None,
                substitute_read_at: None,
                substitute_read_with: None,
                len_pointers: Vec::new(),
                read_pointers: Vec::new(),
            }
        }

        fn pointer(&self) -> ZkAmsMkheDirectObjectPointerV1 {
            self.objects[0].0
        }

        fn insert(
            &mut self,
            kind: ZkAmsMkheDirectObjectKindV1,
            bytes: Vec<u8>,
        ) -> ZkAmsMkheDirectObjectPointerV1 {
            let pointer = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();
            self.objects.push((pointer, bytes));
            pointer
        }

        fn object_index(
            &self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.objects
                .iter()
                .position(|(candidate, _)| *candidate == pointer)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
        }
    }

    impl ZkAmsMkheDirectObjectReadAtProviderV1 for TestProvider {
        fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
            self.identity_calls += 1;
            Ok(
                if self
                    .identity_drift_at
                    .is_some_and(|call| self.identity_calls >= call)
                {
                    DRIFTED_PROVIDER_ID
                } else {
                    self.provider_identity
                },
            )
        }

        fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
            self.snapshot_calls += 1;
            Ok(
                if self
                    .snapshot_drift_at
                    .is_some_and(|call| self.snapshot_calls >= call)
                {
                    DRIFTED_SNAPSHOT_ID
                } else {
                    self.snapshot_identity
                },
            )
        }

        fn object_len(
            &mut self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<u64, ZkAmsMkheErrorV1> {
            self.len_calls += 1;
            self.len_pointers.push(pointer);
            let index = self.object_index(pointer)?;
            let length = u64::try_from(self.objects[index].1.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self.len_drift_at.is_some_and(|call| self.len_calls >= call) {
                return length
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            Ok(length)
        }

        fn read_at(
            &mut self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
            absolute_offset: u64,
            destination: &mut [u8],
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.read_calls += 1;
            self.read_pointers.push(pointer);
            if self
                .panic_read_at
                .is_some_and(|call| self.read_calls == call)
            {
                panic!("simulated untrusted direct-object provider panic");
            }
            let served_pointer = if self
                .substitute_read_at
                .is_some_and(|call| self.read_calls == call)
            {
                self.substitute_read_with
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            } else {
                pointer
            };
            let object_index = self.object_index(served_pointer)?;
            if self
                .mutate_same_snapshot_at
                .is_some_and(|call| self.read_calls == call)
            {
                let index = usize::try_from(absolute_offset)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                let byte = self
                    .objects
                    .get_mut(object_index)
                    .and_then(|(_, bytes)| bytes.get_mut(index))
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
                *byte ^= 0x80;
            }
            let start = usize::try_from(absolute_offset)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let short = self
                .short_read_at
                .is_some_and(|call| self.read_calls == call);
            let copied = if short {
                destination.len().saturating_sub(1)
            } else {
                destination.len()
            };
            let end = start
                .checked_add(copied)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let source = self
                .objects
                .get(object_index)
                .map(|(_, bytes)| bytes)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .get(start..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            destination[..copied].copy_from_slice(source);
            if self
                .over_read_at
                .is_some_and(|call| self.read_calls == call)
            {
                destination
                    .len()
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(copied)
            }
        }
    }

    fn payload(bytes: usize) -> Vec<u8> {
        (0..bytes)
            .map(|index| (index as u8).wrapping_mul(29).wrapping_add(7))
            .collect()
    }

    #[test]
    fn canonical_pointer_roundtrips_and_rejects_every_framing_mutation() {
        for kind in [
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            ZkAmsMkheDirectObjectKindV1::RkgK,
            ZkAmsMkheDirectObjectKindV1::RkgNormalization,
            ZkAmsMkheDirectObjectKindV1::GaloisB,
            ZkAmsMkheDirectObjectKindV1::AggregateH0,
            ZkAmsMkheDirectObjectKindV1::AggregateH1,
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
        ] {
            let pointer =
                ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &payload(257)).unwrap();
            let encoded = pointer.encode();
            assert_eq!(encoded.len(), ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1);
            assert_eq!(&encoded[..4], b"ZDOP");
            assert_eq!(encoded[4], MKHE_VERSION_V1);
            assert_eq!(
                ZkAmsMkheDirectObjectPointerV1::decode_exact(kind, &encoded).unwrap(),
                pointer
            );
            for end in 0..encoded.len() {
                assert!(
                    ZkAmsMkheDirectObjectPointerV1::decode_exact(kind, &encoded[..end]).is_err(),
                    "accepted pointer truncation at {end}"
                );
            }
            let mut trailing = encoded.to_vec();
            trailing.push(0);
            assert!(ZkAmsMkheDirectObjectPointerV1::decode_exact(kind, &trailing).is_err());
            for index in 0..encoded.len() {
                let mut changed = encoded;
                changed[index] ^= 0x80;
                assert!(
                    ZkAmsMkheDirectObjectPointerV1::decode_exact(kind, &changed).is_err(),
                    "accepted pointer mutation at byte {index}"
                );
            }
        }
    }

    #[test]
    fn pointer_shape_rejects_wrong_kind_zeroes_and_resource_overflow() {
        let bytes = payload(31);
        let polynomial = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            &bytes,
        )
        .unwrap();
        let proof = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            &bytes,
        )
        .unwrap();
        let cpk_party_b = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            &bytes,
        )
        .unwrap();
        let cpk_relation = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
            &bytes,
        )
        .unwrap();
        assert!(
            ZkAmsMkheDirectObjectPointerV1::decode_exact(
                ZkAmsMkheDirectObjectKindV1::RkgH0,
                &proof.encode(),
            )
            .is_err()
        );
        assert_ne!(polynomial.pointer_digest(), proof.pointer_digest());
        assert_ne!(cpk_party_b.pointer_digest(), cpk_relation.pointer_digest());
        assert!(
            ZkAmsMkheDirectObjectPointerV1::decode_exact(
                ZkAmsMkheDirectObjectKindV1::CpkPartyB,
                &cpk_relation.encode(),
            )
            .is_err()
        );
        assert_eq!(
            ZkAmsMkheDirectObjectPointerV1::new(ZkAmsMkheDirectObjectKindV1::RkgH0, 0, [1; 32],),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            ZkAmsMkheDirectObjectPointerV1::new(ZkAmsMkheDirectObjectKindV1::RkgH0, 1, [0; 32],),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            ZkAmsMkheDirectObjectPointerV1::new(
                ZkAmsMkheDirectObjectKindV1::RkgH0,
                u64::MAX,
                [1; 32],
            ),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );
    }

    #[test]
    fn complete_chunked_validation_matches_the_content_address() {
        let mut provider = TestProvider::new(
            ZkAmsMkheDirectObjectKindV1::AggregateH0,
            payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 19),
        );
        let pointer = provider.pointer();
        let receipt = validate_zk_ams_mkhe_direct_object_v1(
            ZkAmsMkheDirectObjectKindV1::AggregateH0,
            pointer,
            &mut provider,
        )
        .unwrap();
        receipt.validate().unwrap();
        assert_eq!(receipt.snapshot().pointer(), pointer);
        assert_eq!(receipt.snapshot().provider_identity(), PROVIDER_ID);
        assert_eq!(receipt.snapshot().snapshot_identity(), SNAPSHOT_ID);
        assert_eq!(receipt.canonical_bytes(), pointer.payload_bytes());
        assert_eq!(receipt.payload_blake3(), pointer.payload_blake3());
        assert_ne!(receipt.snapshot().snapshot_binding_digest(), [0; 32]);
        assert_ne!(receipt.receipt_digest(), [0; 32]);
        assert_eq!(provider.read_calls, 3);
        assert!(!ZK_AMS_MKHE_DIRECT_OBJECT_ADMISSION_GATE_V1);
        assert!(!ZK_AMS_MKHE_DIRECT_OBJECT_RELEASE_GATE_V1);
    }

    #[test]
    fn short_and_impossible_over_reads_are_hard_failures_and_poison_the_pass() {
        for over_read in [false, true] {
            let mut provider =
                TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(97));
            let pointer = provider.pointer();
            if over_read {
                provider.over_read_at = Some(1);
            } else {
                provider.short_read_at = Some(1);
            }
            let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
                ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                pointer,
                &mut provider,
            )
            .unwrap();
            let mut buffer = [0_u8; 97];
            assert_eq!(
                transaction.read_next(&mut provider, &mut buffer),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
            provider.short_read_at = None;
            provider.over_read_at = None;
            assert_eq!(
                transaction.read_next(&mut provider, &mut buffer),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
                "a failed pass must not be repaired by retry"
            );
            assert_eq!(
                transaction.finish(&mut provider),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
            assert_eq!(provider.read_calls, 1);
        }
    }

    #[test]
    fn caught_provider_unwind_permanently_poisons_the_pass() {
        let mut provider =
            TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(97));
        provider.panic_read_at = Some(1);
        let pointer = provider.pointer();
        let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            pointer,
            &mut provider,
        )
        .unwrap();
        let mut buffer = [0_u8; 97];

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = transaction.read_next(&mut provider, &mut buffer);
        }));
        assert!(unwind.is_err());

        provider.panic_read_at = None;
        assert_eq!(
            transaction.read_next(&mut provider, &mut buffer),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(provider.read_calls, 1, "poisoned pass reached provider I/O");
        assert_eq!(
            transaction.finish(&mut provider),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
    }

    #[test]
    fn length_provider_and_snapshot_drift_are_rejected_around_reads() {
        let cases = [0_u8, 1, 2];
        for case in cases {
            let mut provider =
                TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(128));
            match case {
                0 => provider.len_drift_at = Some(2),
                1 => provider.identity_drift_at = Some(2),
                2 => provider.snapshot_drift_at = Some(3),
                _ => unreachable!(),
            }
            let pointer = provider.pointer();
            assert_eq!(
                validate_zk_ams_mkhe_direct_object_v1(
                    ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                    pointer,
                    &mut provider,
                ),
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
                "accepted drift case {case}"
            );
            if case == 2 {
                assert_eq!(provider.read_calls, 1, "post-read drift was not exercised");
            } else {
                assert_eq!(
                    provider.read_calls, 0,
                    "pre-read drift reached provider I/O"
                );
            }
        }
    }

    #[test]
    fn stable_snapshot_labels_cannot_hide_payload_mutation_or_wrong_hash() {
        let bytes = payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 5);
        let mut mutating = TestProvider::new(ZkAmsMkheDirectObjectKindV1::RkgK, bytes.clone());
        mutating.mutate_same_snapshot_at = Some(2);
        let pointer = mutating.pointer();
        assert_eq!(
            validate_zk_ams_mkhe_direct_object_v1(
                ZkAmsMkheDirectObjectKindV1::RkgK,
                pointer,
                &mut mutating,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert!(mutating.identity_calls > 1);
        assert!(mutating.snapshot_calls > 1);
        assert_eq!(mutating.identity_calls, mutating.snapshot_calls);

        let wrong_length = u64::try_from(bytes.len()).unwrap();
        let mut wrong_hash = TestProvider::new(ZkAmsMkheDirectObjectKindV1::RkgK, bytes);
        let wrong_pointer = ZkAmsMkheDirectObjectPointerV1::new(
            ZkAmsMkheDirectObjectKindV1::RkgK,
            wrong_length,
            [0xa5; 32],
        )
        .unwrap();
        wrong_hash.objects[0].0 = wrong_pointer;
        let pointer = wrong_hash.pointer();
        assert_eq!(
            validate_zk_ams_mkhe_direct_object_v1(
                ZkAmsMkheDirectObjectKindV1::RkgK,
                pointer,
                &mut wrong_hash,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
    }

    #[test]
    fn multi_object_provider_is_pointer_addressed_and_rejects_cross_object_substitution() {
        let first = payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 11);
        let mut second = first.clone();
        second[ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1] ^= 0x55;
        let mut provider = TestProvider::new(ZkAmsMkheDirectObjectKindV1::RkgH0, first);
        let h0 = provider.pointer();
        let h1 = provider.insert(ZkAmsMkheDirectObjectKindV1::RkgH1, second);
        let receipt = validate_zk_ams_mkhe_direct_object_v1(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            h0,
            &mut provider,
        )
        .unwrap();
        assert_eq!(receipt.snapshot().pointer(), h0);
        assert!(provider.len_pointers.iter().all(|pointer| *pointer == h0));
        assert!(provider.read_pointers.iter().all(|pointer| *pointer == h0));

        let mut substituted = provider.clone();
        substituted.identity_calls = 0;
        substituted.snapshot_calls = 0;
        substituted.len_calls = 0;
        substituted.read_calls = 0;
        substituted.len_pointers.clear();
        substituted.read_pointers.clear();
        substituted.substitute_read_at = Some(2);
        substituted.substitute_read_with = Some(h1);
        assert_eq!(
            validate_zk_ams_mkhe_direct_object_v1(
                ZkAmsMkheDirectObjectKindV1::RkgH0,
                h0,
                &mut substituted,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert!(
            substituted
                .len_pointers
                .iter()
                .all(|pointer| *pointer == h0)
        );
        assert!(
            substituted
                .read_pointers
                .iter()
                .all(|pointer| *pointer == h0)
        );

        let provider_calls = provider.len_calls;
        assert_eq!(
            bind_zk_ams_mkhe_direct_object_snapshot_v1(
                ZkAmsMkheDirectObjectKindV1::RkgH0,
                h1,
                &mut provider,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
            "a valid H1 pointer must not decode as H0"
        );
        assert_eq!(provider.len_calls, provider_calls);

        let same_bytes = payload(211);
        let party_h0 = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            &same_bytes,
        )
        .unwrap();
        let aggregate_h0 = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::AggregateH0,
            &same_bytes,
        )
        .unwrap();
        assert_eq!(party_h0.payload_blake3(), aggregate_h0.payload_blake3());
        assert_ne!(party_h0.pointer_digest(), aggregate_h0.pointer_digest());
        assert_ne!(party_h0, aggregate_h0);

        let mut trailing =
            TestProvider::new(ZkAmsMkheDirectObjectKindV1::RkgNormalization, same_bytes);
        let expected = trailing.pointer();
        trailing.objects[0].1.push(0);
        assert_eq!(
            bind_zk_ams_mkhe_direct_object_snapshot_v1(
                ZkAmsMkheDirectObjectKindV1::RkgNormalization,
                expected,
                &mut trailing,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }

    #[test]
    fn zero_provider_axes_are_rejected() {
        for zero_provider in [true, false] {
            let mut provider =
                TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(9));
            if zero_provider {
                provider.provider_identity = [0; 32];
            } else {
                provider.snapshot_identity = [0; 32];
            }
            let pointer = provider.pointer();
            assert_eq!(
                bind_zk_ams_mkhe_direct_object_snapshot_v1(
                    ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                    pointer,
                    &mut provider,
                ),
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
            );
            assert_eq!(provider.read_calls, 0);
        }
    }

    #[test]
    fn exact_read_bounds_reject_empty_oversized_out_of_range_and_overflow_pre_io() {
        let mut provider =
            TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(64));
        let pointer = provider.pointer();
        let snapshot = bind_zk_ams_mkhe_direct_object_snapshot_v1(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            pointer,
            &mut provider,
        )
        .unwrap();
        let mut empty = [];
        assert_eq!(
            read_zk_ams_mkhe_direct_object_at_exact_v1(snapshot, &mut provider, 0, &mut empty,),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        let mut oversized = vec![0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1];
        assert_eq!(
            read_zk_ams_mkhe_direct_object_at_exact_v1(snapshot, &mut provider, 0, &mut oversized,),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );
        let mut one = [0_u8; 1];
        assert_eq!(
            read_zk_ams_mkhe_direct_object_at_exact_v1(
                snapshot,
                &mut provider,
                pointer.payload_bytes(),
                &mut one,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            read_zk_ams_mkhe_direct_object_at_exact_v1(snapshot, &mut provider, u64::MAX, &mut one,),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(provider.read_calls, 0);
    }

    #[test]
    fn incomplete_and_tampered_receipts_never_validate() {
        let mut provider =
            TestProvider::new(ZkAmsMkheDirectObjectKindV1::ProofEnvelope, payload(96));
        let pointer = provider.pointer();
        let transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            pointer,
            &mut provider,
        )
        .unwrap();
        assert_eq!(
            transaction.finish(&mut provider),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );

        let mut receipt = validate_zk_ams_mkhe_direct_object_v1(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            pointer,
            &mut provider,
        )
        .unwrap();
        receipt.receipt_digest[0] ^= 1;
        assert_eq!(
            receipt.validate(),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
    }
}
