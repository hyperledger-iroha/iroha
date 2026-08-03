//! Content-addressed object transport for direct-relation verification.
//!
//! A sound direct-relation verifier needs the actual canonical public RNS
//! polynomials, not only digests copied into a statement.  Those polynomials
//! are too large to inline in a proof envelope, so this module establishes the
//! phase-one boundary for publishing them into and reading them from an
//! immutable content-addressed object provider.
//!
//! Publication uses move-only staging authority, an atomically consumed seal,
//! a complete bounded reread of immutable sealed bytes, atomic idempotent
//! publication by the resulting pointer, authoritative lost-ack lookup, and a
//! second complete readback through the published provider. There is no abort,
//! delete, or unpublish transition at this boundary.
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
const DIRECT_OBJECT_STAGING_TOKEN_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-staging-token";
const DIRECT_OBJECT_SEAL_TOKEN_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-seal-token";
const DIRECT_OBJECT_PUBLISHED_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-published-binding";
const DIRECT_OBJECT_PUBLICATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-publication-receipt";

/// Fixed width of one canonical direct-object pointer frame.
pub const ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1: usize = 4 + 1 + 1 + 8 + 32 + 32;
/// Sole maximum request passed to an untrusted direct-object provider.
pub const ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1: usize = 8 * 1024;
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
pub enum ZkAmsMkheDirectObjectKindV1 {
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
pub struct ZkAmsMkheDirectObjectPointerV1 {
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
    pointer_digest: [u8; 32],
}

impl ZkAmsMkheDirectObjectPointerV1 {
    /// Construct a canonical pointer from an independently computed content address.
    pub fn new(
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
    pub fn from_payload(
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
    pub fn encode(self) -> [u8; ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1] {
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
    pub fn decode_exact(
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
    pub const fn kind(self) -> ZkAmsMkheDirectObjectKindV1 {
        self.kind
    }

    /// Exact complete object length.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }

    /// BLAKE3 digest of every byte in the exact complete object.
    #[must_use]
    pub const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }

    /// Domain-separated digest of the complete canonical pointer frame.
    #[must_use]
    pub const fn pointer_digest(self) -> [u8; 32] {
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

/// Move-only authority for one unpublished direct-object staging allocation.
///
/// The backend must issue a globally unique `staging_identity` for every
/// successful allocation in one publication namespace. The canonical adapter
/// additionally binds that identity to the exact publication session, object
/// kind, and length. A staging token is deliberately neither `Clone` nor
/// `Copy`; sealing consumes it.
pub struct ZkAmsMkheDirectObjectStagingTokenV1 {
    publication_identity: [u8; 32],
    staging_identity: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    token_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheDirectObjectStagingTokenV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectStagingTokenV1")
            .field(
                "publication_identity",
                &hex::encode(self.publication_identity),
            )
            .field("staging_identity", &hex::encode(self.staging_identity))
            .field("kind", &self.kind)
            .field("payload_bytes", &self.payload_bytes)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectObjectStagingTokenV1 {
    /// Construct one backend-issued staging token under exact publication axes.
    pub fn new(
        publication_identity: [u8; 32],
        staging_identity: [u8; 32],
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut token = Self {
            publication_identity,
            staging_identity,
            kind,
            payload_bytes,
            token_digest: [0; 32],
        };
        token.token_digest = direct_object_staging_token_digest(&token);
        token.validate()?;
        Ok(token)
    }

    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.publication_identity == [0; 32]
            || self.staging_identity == [0; 32]
            || self.staging_identity == self.publication_identity
            || self.payload_bytes == 0
            || self.payload_bytes > self.kind.payload_ceiling()
            || self.token_digest == [0; 32]
            || self.token_digest != direct_object_staging_token_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Exact publication session which allocated this stage.
    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    /// Backend-unique identity of this exact unpublished stage.
    #[must_use]
    pub const fn staging_identity(&self) -> [u8; 32] {
        self.staging_identity
    }

    /// Exact object kind accepted by this stage.
    #[must_use]
    pub const fn kind(&self) -> ZkAmsMkheDirectObjectKindV1 {
        self.kind
    }

    /// Exact complete payload length accepted by this stage.
    #[must_use]
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }

    /// Digest binding every staging-token axis.
    #[must_use]
    pub const fn token_digest(&self) -> [u8; 32] {
        self.token_digest
    }
}

fn direct_object_staging_token_digest(token: &ZkAmsMkheDirectObjectStagingTokenV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_STAGING_TOKEN_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1, token.kind as u8]);
    hash.update(&token.publication_identity);
    hash.update(&token.staging_identity);
    hash.update(&token.payload_bytes.to_be_bytes());
    hash.finalize()
}

/// Move-only authority for one immutable, completely written staging object.
///
/// A backend constructs this token only by consuming the matching staging
/// token. The distinct seal identity prevents a mutable-stage handle from
/// being confused with immutable sealed storage. Publication borrows this
/// token so an ambiguous commit can be reconciled without inventing an abort
/// or unpublish operation.
pub struct ZkAmsMkheDirectObjectSealTokenV1 {
    publication_identity: [u8; 32],
    staging_identity: [u8; 32],
    staging_token_digest: [u8; 32],
    seal_identity: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    token_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheDirectObjectSealTokenV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectSealTokenV1")
            .field(
                "publication_identity",
                &hex::encode(self.publication_identity),
            )
            .field("staging_identity", &hex::encode(self.staging_identity))
            .field("seal_identity", &hex::encode(self.seal_identity))
            .field("kind", &self.kind)
            .field("payload_bytes", &self.payload_bytes)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectObjectSealTokenV1 {
    /// Consume mutable staging authority and bind a backend seal identity.
    pub fn from_staging(
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
        seal_identity: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        staging.validate()?;
        let mut token = Self {
            publication_identity: staging.publication_identity,
            staging_identity: staging.staging_identity,
            staging_token_digest: staging.token_digest,
            seal_identity,
            kind: staging.kind,
            payload_bytes: staging.payload_bytes,
            token_digest: [0; 32],
        };
        token.token_digest = direct_object_seal_token_digest(&token);
        token.validate()?;
        Ok(token)
    }

    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.publication_identity == [0; 32]
            || self.staging_identity == [0; 32]
            || self.seal_identity == [0; 32]
            || self.staging_identity == self.publication_identity
            || self.seal_identity == self.publication_identity
            || self.seal_identity == self.staging_identity
            || self.staging_token_digest == [0; 32]
            || self.payload_bytes == 0
            || self.payload_bytes > self.kind.payload_ceiling()
            || self.token_digest == [0; 32]
            || self.token_digest != direct_object_seal_token_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Exact publication session which owns this seal.
    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    /// Identity of the exact consumed stage.
    #[must_use]
    pub const fn staging_identity(&self) -> [u8; 32] {
        self.staging_identity
    }

    /// Digest of the exact consumed staging token.
    #[must_use]
    pub const fn staging_token_digest(&self) -> [u8; 32] {
        self.staging_token_digest
    }

    /// Backend-unique identity of the immutable sealed object.
    #[must_use]
    pub const fn seal_identity(&self) -> [u8; 32] {
        self.seal_identity
    }

    /// Exact sealed object kind.
    #[must_use]
    pub const fn kind(&self) -> ZkAmsMkheDirectObjectKindV1 {
        self.kind
    }

    /// Exact sealed payload length.
    #[must_use]
    pub const fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }

    /// Digest binding the stage lineage and every seal axis.
    #[must_use]
    pub const fn token_digest(&self) -> [u8; 32] {
        self.token_digest
    }
}

fn direct_object_seal_token_digest(token: &ZkAmsMkheDirectObjectSealTokenV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_SEAL_TOKEN_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1, token.kind as u8]);
    hash.update(&token.publication_identity);
    hash.update(&token.staging_identity);
    hash.update(&token.staging_token_digest);
    hash.update(&token.seal_identity);
    hash.update(&token.payload_bytes.to_be_bytes());
    hash.finalize()
}

/// Authoritative immutable binding observed by exact pointer lookup.
///
/// The published-object identity is backend-local provenance, not a content
/// address. Soundness comes from the exact pointer and the mandatory complete
/// post-publication readback; this binding proves only that authoritative
/// lookup observed an immutable entry in the current publication session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDirectObjectPublishedBindingV1 {
    publication_identity: [u8; 32],
    published_object_identity: [u8; 32],
    pointer: ZkAmsMkheDirectObjectPointerV1,
    binding_digest: [u8; 32],
}

impl ZkAmsMkheDirectObjectPublishedBindingV1 {
    /// Construct one authoritative lookup result for an exact pointer.
    pub fn new(
        publication_identity: [u8; 32],
        published_object_identity: [u8; 32],
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        pointer.validate_for_kind(pointer.kind)?;
        let mut binding = Self {
            publication_identity,
            published_object_identity,
            pointer,
            binding_digest: [0; 32],
        };
        binding.binding_digest = direct_object_published_binding_digest(binding);
        binding.validate()?;
        Ok(binding)
    }

    fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        self.pointer.validate_for_kind(self.pointer.kind)?;
        if self.publication_identity == [0; 32]
            || self.published_object_identity == [0; 32]
            || self.binding_digest == [0; 32]
            || self.binding_digest != direct_object_published_binding_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Current publication session which performed authoritative lookup.
    #[must_use]
    pub const fn publication_identity(self) -> [u8; 32] {
        self.publication_identity
    }

    /// Backend-local immutable-object identity.
    #[must_use]
    pub const fn published_object_identity(self) -> [u8; 32] {
        self.published_object_identity
    }

    /// Exact typed content address found by lookup.
    #[must_use]
    pub const fn pointer(self) -> ZkAmsMkheDirectObjectPointerV1 {
        self.pointer
    }

    /// Digest binding every authoritative lookup axis.
    #[must_use]
    pub const fn binding_digest(self) -> [u8; 32] {
        self.binding_digest
    }
}

fn direct_object_published_binding_digest(
    binding: ZkAmsMkheDirectObjectPublishedBindingV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_PUBLISHED_BINDING_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&binding.publication_identity);
    hash.update(&binding.published_object_identity);
    hash.update(&binding.pointer.pointer_digest);
    hash.finalize()
}

/// Absolute-offset CAS publication backend for immutable direct objects.
///
/// Staging storage is never visible through
/// [`ZkAmsMkheDirectObjectReadAtProviderV1`]. `seal_staged` atomically consumes
/// mutable staging authority and freezes its bytes. `publish_sealed_by_pointer`
/// atomically installs the exact pointer or succeeds idempotently when that
/// pointer already names identical immutable content; it must never overwrite
/// another entry. Its error is potentially ambiguous, so callers always use
/// `lookup_published_pointer` before deciding whether publication occurred.
/// The read-provider supertrait is mandatory because publication is incomplete
/// until the installed pointer passes a full independent provider readback.
///
/// This trait deliberately has no abort, discard, delete, or unpublish method.
/// Incomplete staging and orphaned seals are backend garbage-collection
/// concerns, not security transitions.
pub trait ZkAmsMkheDirectObjectCasPublicationV1: ZkAmsMkheDirectObjectReadAtProviderV1 {
    /// Nonzero identity of this exact open publication session.
    fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1>;

    /// Allocate one empty, unpublished stage for exact kind and length.
    fn begin_staging(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1>;

    /// Return bytes currently written to the exact stage.
    fn staged_len(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1>;

    /// Perform one absolute, non-retrying write to unpublished staging.
    fn write_staged_at(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1>;

    /// Atomically consume mutable staging authority and freeze its exact bytes.
    fn seal_staged(
        &mut self,
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1>;

    /// Return the immutable sealed object's exact logical length.
    fn sealed_len(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1>;

    /// Perform one absolute, non-retrying read from immutable sealed storage.
    fn read_sealed_at(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1>;

    /// Atomically and idempotently install the sealed bytes at the exact pointer.
    ///
    /// An error may be a lost acknowledgement after successful installation.
    fn publish_sealed_by_pointer(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    /// Authoritatively look up one exact immutable pointer after publication.
    fn lookup_published_pointer(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1>;
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
pub trait ZkAmsMkheDirectObjectReadAtProviderV1 {
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
pub struct ZkAmsMkheDirectObjectReadReceiptV1 {
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
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }

    /// Independently recomputed BLAKE3 digest of the complete byte stream.
    #[must_use]
    pub const fn payload_blake3(&self) -> [u8; 32] {
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
pub fn validate_zk_ams_mkhe_direct_object_v1<P>(
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

/// Move-only proof that one sealed object was authoritatively published and reread.
///
/// This receipt has no decoder and cannot be reconstructed from a pointer or
/// lookup result alone. It owns the complete post-publication read receipt, so
/// callers cannot mistake a commit acknowledgement for validation of the
/// bytes actually served through the published-object provider.
pub struct ZkAmsMkheDirectObjectPublicationReceiptV1 {
    publication_identity: [u8; 32],
    staging_identity: [u8; 32],
    staging_token_digest: [u8; 32],
    seal_identity: [u8; 32],
    seal_token_digest: [u8; 32],
    pointer: ZkAmsMkheDirectObjectPointerV1,
    published_binding: ZkAmsMkheDirectObjectPublishedBindingV1,
    post_publish_read_receipt: ZkAmsMkheDirectObjectReadReceiptV1,
    reconciled_after_publish_error: bool,
    receipt_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheDirectObjectPublicationReceiptV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectPublicationReceiptV1")
            .field("pointer", &self.pointer)
            .field(
                "reconciled_after_publish_error",
                &self.reconciled_after_publish_error,
            )
            .field("receipt_digest", &hex::encode(self.receipt_digest))
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectObjectPublicationReceiptV1 {
    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.pointer.validate_for_kind(self.pointer.kind)?;
        self.published_binding.validate()?;
        self.post_publish_read_receipt.validate()?;
        let staging = ZkAmsMkheDirectObjectStagingTokenV1::new(
            self.publication_identity,
            self.staging_identity,
            self.pointer.kind,
            self.pointer.payload_bytes,
        )?;
        if staging.token_digest() != self.staging_token_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let seal = ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, self.seal_identity)?;
        if seal.token_digest() != self.seal_token_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        if self.publication_identity == [0; 32]
            || self.staging_identity == [0; 32]
            || self.staging_token_digest == [0; 32]
            || self.seal_identity == [0; 32]
            || self.seal_token_digest == [0; 32]
            || self.publication_identity == self.staging_identity
            || self.publication_identity == self.seal_identity
            || self.staging_identity == self.seal_identity
            || self.published_binding.publication_identity() != self.publication_identity
            || self.published_binding.pointer() != self.pointer
            || self.post_publish_read_receipt.snapshot().pointer() != self.pointer
            || self.post_publish_read_receipt.canonical_bytes() != self.pointer.payload_bytes
            || self.post_publish_read_receipt.payload_blake3() != self.pointer.payload_blake3
            || self.receipt_digest == [0; 32]
            || self.receipt_digest != direct_object_publication_receipt_digest(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    /// Exact typed content address proven published and readable.
    #[must_use]
    pub const fn pointer(&self) -> ZkAmsMkheDirectObjectPointerV1 {
        self.pointer
    }

    /// Exact publication session which performed the operation and lookup.
    #[must_use]
    pub const fn publication_identity(&self) -> [u8; 32] {
        self.publication_identity
    }

    /// Backend-unique identity of the consumed mutable stage.
    #[must_use]
    pub const fn staging_identity(&self) -> [u8; 32] {
        self.staging_identity
    }

    /// Backend-unique identity of the immutable sealed stage.
    #[must_use]
    pub const fn seal_identity(&self) -> [u8; 32] {
        self.seal_identity
    }

    /// Authoritative pointer lookup retained by the publication receipt.
    #[must_use]
    pub const fn published_binding(&self) -> ZkAmsMkheDirectObjectPublishedBindingV1 {
        self.published_binding
    }

    /// Complete content-hash receipt from the published provider.
    #[must_use]
    pub const fn post_publish_read_receipt(&self) -> &ZkAmsMkheDirectObjectReadReceiptV1 {
        &self.post_publish_read_receipt
    }

    /// Whether an error acknowledgement was reconciled to an exact published pointer.
    #[must_use]
    pub const fn reconciled_after_publish_error(&self) -> bool {
        self.reconciled_after_publish_error
    }

    /// Digest binding every publication, lookup, and readback axis.
    #[must_use]
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
}

fn direct_object_publication_receipt_digest(
    receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_OBJECT_PUBLICATION_RECEIPT_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1, receipt.pointer.kind as u8]);
    hash.update(&receipt.publication_identity);
    hash.update(&receipt.staging_identity);
    hash.update(&receipt.staging_token_digest);
    hash.update(&receipt.seal_identity);
    hash.update(&receipt.seal_token_digest);
    hash.update(&receipt.pointer.pointer_digest);
    hash.update(&receipt.published_binding.binding_digest);
    hash.update(&receipt.post_publish_read_receipt.receipt_digest);
    hash.update(&[u8::from(receipt.reconciled_after_publish_error)]);
    hash.finalize()
}

fn ensure_direct_object_publication_identity_v1<P>(
    publisher: &mut P,
    expected: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let observed = publisher.publication_identity()?;
    if expected == [0; 32] || observed != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn observe_direct_object_staged_len_v1<P>(
    publisher: &mut P,
    expected_publication_identity: [u8; 32],
    staging: &ZkAmsMkheDirectObjectStagingTokenV1,
) -> Result<u64, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    staging.validate()?;
    ensure_direct_object_publication_identity_v1(publisher, expected_publication_identity)?;
    let length = publisher.staged_len(staging);
    let stable =
        ensure_direct_object_publication_identity_v1(publisher, expected_publication_identity);
    let length = length?;
    stable?;
    Ok(length)
}

fn observe_direct_object_sealed_len_v1<P>(
    publisher: &mut P,
    expected_publication_identity: [u8; 32],
    seal: &ZkAmsMkheDirectObjectSealTokenV1,
) -> Result<u64, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    seal.validate()?;
    ensure_direct_object_publication_identity_v1(publisher, expected_publication_identity)?;
    let length = publisher.sealed_len(seal);
    let stable =
        ensure_direct_object_publication_identity_v1(publisher, expected_publication_identity);
    let length = length?;
    stable?;
    Ok(length)
}

/// Single-use absolute-offset transaction for one immutable CAS publication.
///
/// Every invalid or failed write permanently poisons the transaction. Finishing
/// consumes the staging token, rereads only immutable sealed storage to derive
/// the content address, publishes atomically, reconciles the authoritative
/// pointer lookup even after an error acknowledgement, and finally validates a
/// complete read through the published-object provider.
///
/// This type intentionally has no `Drop` implementation. In particular, a
/// failed post-publish check can never invoke backend cleanup that might race a
/// successful but ambiguously acknowledged commit.
pub struct ZkAmsMkheDirectObjectPublicationTransactionV1<
    'a,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
> {
    publisher: &'a mut P,
    staging: Option<ZkAmsMkheDirectObjectStagingTokenV1>,
    publication_identity: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
    next_offset: u64,
    expected_payload_hasher: norito::streaming::Blake3Hasher,
    failed: bool,
}

impl<P> core::fmt::Debug for ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectPublicationTransactionV1")
            .field(
                "publication_identity",
                &hex::encode(self.publication_identity),
            )
            .field("kind", &self.kind)
            .field("payload_bytes", &self.payload_bytes)
            .field("next_offset", &self.next_offset)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl<'a, P> ZkAmsMkheDirectObjectPublicationTransactionV1<'a, P>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    /// Allocate one exact empty stage before writing any payload byte.
    pub fn begin(
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
        publisher: &'a mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if payload_bytes == 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if payload_bytes > kind.payload_ceiling() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let publication_identity = publisher.publication_identity()?;
        if publication_identity == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let staging = publisher.begin_staging(kind, payload_bytes)?;
        ensure_direct_object_publication_identity_v1(publisher, publication_identity)?;
        staging.validate()?;
        if staging.publication_identity != publication_identity
            || staging.kind != kind
            || staging.payload_bytes != payload_bytes
            || observe_direct_object_staged_len_v1(publisher, publication_identity, &staging)? != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            publisher,
            staging: Some(staging),
            publication_identity,
            kind,
            payload_bytes,
            next_offset: 0,
            expected_payload_hasher: norito::streaming::Blake3Hasher::new(),
            failed: false,
        })
    }

    /// Append one exact bounded chunk at the sole next absolute offset.
    ///
    /// Empty, oversized, out-of-range, short, over-reported, failed, and
    /// panicking backend writes all leave this transaction permanently poisoned.
    pub fn write_exact(&mut self, source: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        // Poison before validation and before entering backend-controlled code.
        // A caught unwind must not permit a second write under the same token.
        self.failed = true;
        let result = self.write_exact_inner(source);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    fn write_exact_inner(&mut self, source: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        if source.is_empty() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if source.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let requested =
            u64::try_from(source.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let after = self
            .next_offset
            .checked_add(requested)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if after > self.payload_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let staging = self
            .staging
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if observe_direct_object_staged_len_v1(self.publisher, self.publication_identity, staging)?
            != self.next_offset
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity)?;
        let written = self
            .publisher
            .write_staged_at(staging, self.next_offset, source);
        let stable =
            ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity);
        let written = written?;
        stable?;
        if written != source.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if observe_direct_object_staged_len_v1(self.publisher, self.publication_identity, staging)?
            != after
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.expected_payload_hasher.update(source);
        self.next_offset = after;
        Ok(())
    }

    /// Bytes still required before sealing can begin.
    #[must_use]
    pub const fn remaining_bytes(&self) -> u64 {
        self.payload_bytes - self.next_offset
    }

    /// Seal, authenticate, publish, reconcile, and fully reread this object.
    pub fn finish(mut self) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1> {
        if self.failed || self.next_offset != self.payload_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let staging = self
            .staging
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        staging.validate()?;
        if staging.publication_identity != self.publication_identity
            || staging.kind != self.kind
            || staging.payload_bytes != self.payload_bytes
            || observe_direct_object_staged_len_v1(
                self.publisher,
                self.publication_identity,
                &staging,
            )? != self.payload_bytes
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let staging_identity = staging.staging_identity;
        let staging_token_digest = staging.token_digest;
        ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity)?;
        let seal = self.publisher.seal_staged(staging)?;
        ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity)?;
        seal.validate()?;
        if seal.publication_identity != self.publication_identity
            || seal.staging_identity != staging_identity
            || seal.staging_token_digest != staging_token_digest
            || seal.kind != self.kind
            || seal.payload_bytes != self.payload_bytes
            || observe_direct_object_sealed_len_v1(
                self.publisher,
                self.publication_identity,
                &seal,
            )? != self.payload_bytes
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }

        // Only this immutable sealed-stage reread determines the content
        // address. The separately accumulated source hash merely proves that
        // the backend sealed the exact bytes accepted from the caller.
        let mut payload_hasher = norito::streaming::Blake3Hasher::new();
        let mut absolute_offset = 0_u64;
        let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
        while absolute_offset != self.payload_bytes {
            let remaining = self
                .payload_bytes
                .checked_sub(absolute_offset)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let take = buffer.len().min(
                usize::try_from(remaining)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            );
            if take == 0
                || observe_direct_object_sealed_len_v1(
                    self.publisher,
                    self.publication_identity,
                    &seal,
                )? != self.payload_bytes
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            ensure_direct_object_publication_identity_v1(
                self.publisher,
                self.publication_identity,
            )?;
            let read = self
                .publisher
                .read_sealed_at(&seal, absolute_offset, &mut buffer[..take]);
            let stable = ensure_direct_object_publication_identity_v1(
                self.publisher,
                self.publication_identity,
            );
            let read = read?;
            stable?;
            if read != take
                || observe_direct_object_sealed_len_v1(
                    self.publisher,
                    self.publication_identity,
                    &seal,
                )? != self.payload_bytes
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            payload_hasher.update(&buffer[..take]);
            absolute_offset = absolute_offset
                .checked_add(
                    u64::try_from(take).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                )
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        }
        let sealed_payload_blake3 = payload_hasher.finalize();
        let expected_payload_blake3 = core::mem::take(&mut self.expected_payload_hasher).finalize();
        if sealed_payload_blake3 != expected_payload_blake3 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let pointer = ZkAmsMkheDirectObjectPointerV1::new(
            self.kind,
            self.payload_bytes,
            sealed_payload_blake3,
        )?;

        ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity)?;
        let publish_failed = self
            .publisher
            .publish_sealed_by_pointer(&seal, pointer)
            .is_err();

        // Lookup is deliberately unconditional after a returned publish result.
        // It is the sole recovery path for a successful commit whose
        // acknowledgement was lost.
        let lookup = self.publisher.lookup_published_pointer(pointer);
        let stable_after_lookup =
            ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity);
        let published_binding = lookup?.ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        stable_after_lookup?;
        published_binding.validate()?;
        if published_binding.publication_identity != self.publication_identity
            || published_binding.pointer != pointer
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }

        let post_publish_read_receipt =
            validate_zk_ams_mkhe_direct_object_v1(self.kind, pointer, self.publisher)?;
        ensure_direct_object_publication_identity_v1(self.publisher, self.publication_identity)?;

        let mut receipt = ZkAmsMkheDirectObjectPublicationReceiptV1 {
            publication_identity: self.publication_identity,
            staging_identity,
            staging_token_digest,
            seal_identity: seal.seal_identity,
            seal_token_digest: seal.token_digest,
            pointer,
            published_binding,
            post_publish_read_receipt,
            reconciled_after_publish_error: publish_failed,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = direct_object_publication_receipt_digest(&receipt);
        receipt.validate()?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    const PROVIDER_ID: [u8; 32] = [0x51; 32];
    const DRIFTED_PROVIDER_ID: [u8; 32] = [0x52; 32];
    const SNAPSHOT_ID: [u8; 32] = [0x61; 32];
    const DRIFTED_SNAPSHOT_ID: [u8; 32] = [0x62; 32];
    const PUBLICATION_ID: [u8; 32] = [0x71; 32];
    const DRIFTED_PUBLICATION_ID: [u8; 32] = [0x72; 32];

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

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestPublishFailure {
        None,
        BeforeCommit,
        AfterCommit,
    }

    struct TestStagingObject {
        token_digest: [u8; 32],
        expected_bytes: u64,
        bytes: Vec<u8>,
    }

    struct TestSealedObject {
        token_digest: [u8; 32],
        bytes: Arc<[u8]>,
    }

    struct TestPublishedObject {
        pointer: ZkAmsMkheDirectObjectPointerV1,
        published_object_identity: [u8; 32],
        bytes: Arc<[u8]>,
    }

    struct TestCasStore {
        publication_identity: [u8; 32],
        reported_publication_identity: [u8; 32],
        provider_identity: [u8; 32],
        snapshot_identity: [u8; 32],
        next_identity: u64,
        staging: Option<TestStagingObject>,
        sealed: Option<TestSealedObject>,
        published: Vec<TestPublishedObject>,
        write_calls: usize,
        sealed_read_calls: usize,
        publish_calls: usize,
        lookup_calls: usize,
        provider_read_calls: usize,
        write_offsets: Vec<u64>,
        sealed_read_offsets: Vec<u64>,
        provider_read_offsets: Vec<u64>,
        max_write: usize,
        max_sealed_read: usize,
        max_provider_read: usize,
        short_write_at: Option<usize>,
        over_write_at: Option<usize>,
        error_write_at: Option<usize>,
        error_write_after_mutation_at: Option<usize>,
        panic_write_at: Option<usize>,
        mutate_write_at: Option<usize>,
        drift_publication_identity_after_write: bool,
        staged_len_bias: bool,
        short_sealed_read_at: Option<usize>,
        over_sealed_read_at: Option<usize>,
        mutate_sealed_read_at: Option<usize>,
        panic_sealed_read_at: Option<usize>,
        sealed_len_bias: bool,
        corrupt_staging_token: bool,
        corrupt_seal_lineage: bool,
        alias_seal_identity: bool,
        drift_publication_identity_after_seal: bool,
        publish_failure: TestPublishFailure,
        panic_publish_after_commit: bool,
        drift_publication_identity_after_publish: bool,
        lookup_error: bool,
        panic_lookup: bool,
        lookup_none: bool,
        lookup_substitute_pointer: Option<ZkAmsMkheDirectObjectPointerV1>,
        corrupt_lookup_binding: bool,
        provider_short_read_at: Option<usize>,
        provider_over_read_at: Option<usize>,
        provider_mutate_read_at: Option<usize>,
        panic_provider_read_at: Option<usize>,
        provider_snapshot_drift_at: Option<usize>,
        provider_len_bias: bool,
    }

    impl TestCasStore {
        fn new() -> Self {
            Self {
                publication_identity: PUBLICATION_ID,
                reported_publication_identity: PUBLICATION_ID,
                provider_identity: PROVIDER_ID,
                snapshot_identity: SNAPSHOT_ID,
                next_identity: 1,
                staging: None,
                sealed: None,
                published: Vec::new(),
                write_calls: 0,
                sealed_read_calls: 0,
                publish_calls: 0,
                lookup_calls: 0,
                provider_read_calls: 0,
                write_offsets: Vec::new(),
                sealed_read_offsets: Vec::new(),
                provider_read_offsets: Vec::new(),
                max_write: 0,
                max_sealed_read: 0,
                max_provider_read: 0,
                short_write_at: None,
                over_write_at: None,
                error_write_at: None,
                error_write_after_mutation_at: None,
                panic_write_at: None,
                mutate_write_at: None,
                drift_publication_identity_after_write: false,
                staged_len_bias: false,
                short_sealed_read_at: None,
                over_sealed_read_at: None,
                mutate_sealed_read_at: None,
                panic_sealed_read_at: None,
                sealed_len_bias: false,
                corrupt_staging_token: false,
                corrupt_seal_lineage: false,
                alias_seal_identity: false,
                drift_publication_identity_after_seal: false,
                publish_failure: TestPublishFailure::None,
                panic_publish_after_commit: false,
                drift_publication_identity_after_publish: false,
                lookup_error: false,
                panic_lookup: false,
                lookup_none: false,
                lookup_substitute_pointer: None,
                corrupt_lookup_binding: false,
                provider_short_read_at: None,
                provider_over_read_at: None,
                provider_mutate_read_at: None,
                panic_provider_read_at: None,
                provider_snapshot_drift_at: None,
                provider_len_bias: false,
            }
        }

        fn fresh_identity(&mut self, tag: u8) -> [u8; 32] {
            let mut identity = [tag; 32];
            identity[24..].copy_from_slice(&self.next_identity.to_be_bytes());
            self.next_identity = self.next_identity.checked_add(1).expect("test identity");
            identity
        }

        fn published_index(
            &self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.published
                .iter()
                .position(|entry| entry.pointer == pointer)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
        }
    }

    impl ZkAmsMkheDirectObjectCasPublicationV1 for TestCasStore {
        fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
            Ok(self.reported_publication_identity)
        }

        fn begin_staging(
            &mut self,
            kind: ZkAmsMkheDirectObjectKindV1,
            payload_bytes: u64,
        ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1> {
            if self.staging.is_some() {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let staging_identity = self.fresh_identity(0x81);
            let mut token = ZkAmsMkheDirectObjectStagingTokenV1::new(
                self.publication_identity,
                staging_identity,
                kind,
                payload_bytes,
            )?;
            self.staging = Some(TestStagingObject {
                token_digest: token.token_digest(),
                expected_bytes: payload_bytes,
                bytes: Vec::new(),
            });
            if self.corrupt_staging_token {
                token.staging_identity = token.publication_identity;
                token.token_digest = direct_object_staging_token_digest(&token);
            }
            Ok(token)
        }

        fn staged_len(
            &mut self,
            staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        ) -> Result<u64, ZkAmsMkheErrorV1> {
            staging.validate()?;
            let stored = self
                .staging
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != staging.token_digest()
                || stored.expected_bytes != staging.payload_bytes()
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let length = u64::try_from(stored.bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self.staged_len_bias {
                length
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(length)
            }
        }

        fn write_staged_at(
            &mut self,
            staging: &ZkAmsMkheDirectObjectStagingTokenV1,
            absolute_offset: u64,
            source: &[u8],
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.write_calls += 1;
            self.write_offsets.push(absolute_offset);
            self.max_write = self.max_write.max(source.len());
            if self.panic_write_at == Some(self.write_calls) {
                panic!("simulated CAS staging write panic");
            }
            if self.error_write_at == Some(self.write_calls) {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            staging.validate()?;
            let stored = self
                .staging
                .as_mut()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != staging.token_digest()
                || usize::try_from(absolute_offset).ok() != Some(stored.bytes.len())
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let accepted = if self.short_write_at == Some(self.write_calls) {
                source.len().saturating_sub(1)
            } else {
                source.len()
            };
            let before = stored.bytes.len();
            stored.bytes.extend_from_slice(&source[..accepted]);
            if self.mutate_write_at == Some(self.write_calls) && accepted != 0 {
                stored.bytes[before + accepted - 1] ^= 0x20;
            }
            if self.drift_publication_identity_after_write {
                self.reported_publication_identity = DRIFTED_PUBLICATION_ID;
            }
            if self.error_write_after_mutation_at == Some(self.write_calls) {
                return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
            }
            if self.over_write_at == Some(self.write_calls) {
                source
                    .len()
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(accepted)
            }
        }

        fn seal_staged(
            &mut self,
            staging: ZkAmsMkheDirectObjectStagingTokenV1,
        ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1> {
            staging.validate()?;
            let stored = self
                .staging
                .take()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != staging.token_digest()
                || u64::try_from(stored.bytes.len()).ok() != Some(stored.expected_bytes)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let seal_identity = self.fresh_identity(0x91);
            let mut seal = ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, seal_identity)?;
            if self.corrupt_seal_lineage {
                seal.staging_token_digest[0] ^= 1;
                seal.token_digest = direct_object_seal_token_digest(&seal);
            }
            if self.alias_seal_identity {
                seal.seal_identity = seal.staging_identity;
                seal.token_digest = direct_object_seal_token_digest(&seal);
            }
            self.sealed = Some(TestSealedObject {
                token_digest: seal.token_digest(),
                bytes: Arc::from(stored.bytes),
            });
            if self.drift_publication_identity_after_seal {
                self.reported_publication_identity = DRIFTED_PUBLICATION_ID;
            }
            Ok(seal)
        }

        fn sealed_len(
            &mut self,
            seal: &ZkAmsMkheDirectObjectSealTokenV1,
        ) -> Result<u64, ZkAmsMkheErrorV1> {
            seal.validate()?;
            let stored = self
                .sealed
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != seal.token_digest() {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let length = u64::try_from(stored.bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self.sealed_len_bias {
                length
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(length)
            }
        }

        fn read_sealed_at(
            &mut self,
            seal: &ZkAmsMkheDirectObjectSealTokenV1,
            absolute_offset: u64,
            destination: &mut [u8],
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.sealed_read_calls += 1;
            self.sealed_read_offsets.push(absolute_offset);
            self.max_sealed_read = self.max_sealed_read.max(destination.len());
            if self.panic_sealed_read_at == Some(self.sealed_read_calls) {
                panic!("simulated immutable sealed read panic");
            }
            seal.validate()?;
            let stored = self
                .sealed
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != seal.token_digest() {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let start = usize::try_from(absolute_offset)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let copied = if self.short_sealed_read_at == Some(self.sealed_read_calls) {
                destination.len().saturating_sub(1)
            } else {
                destination.len()
            };
            let end = start
                .checked_add(copied)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let source = stored
                .bytes
                .get(start..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            destination[..copied].copy_from_slice(source);
            if self.mutate_sealed_read_at == Some(self.sealed_read_calls) && copied != 0 {
                destination[copied - 1] ^= 0x80;
            }
            if self.over_sealed_read_at == Some(self.sealed_read_calls) {
                destination
                    .len()
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(copied)
            }
        }

        fn publish_sealed_by_pointer(
            &mut self,
            seal: &ZkAmsMkheDirectObjectSealTokenV1,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<(), ZkAmsMkheErrorV1> {
            self.publish_calls += 1;
            seal.validate()?;
            let stored = self
                .sealed
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if stored.token_digest != seal.token_digest() {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let sealed_bytes = Arc::clone(&stored.bytes);
            let expected =
                ZkAmsMkheDirectObjectPointerV1::from_payload(seal.kind(), sealed_bytes.as_ref())?;
            if expected != pointer {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            if self.publish_failure == TestPublishFailure::BeforeCommit {
                return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
            }
            if let Some(existing) = self.published.iter().find(|entry| entry.pointer == pointer) {
                if existing.bytes.as_ref() != sealed_bytes.as_ref() {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            } else {
                let published_object_identity = self.fresh_identity(0xa1);
                self.published.push(TestPublishedObject {
                    pointer,
                    published_object_identity,
                    bytes: sealed_bytes,
                });
            }
            if self.panic_publish_after_commit {
                panic!("simulated lost acknowledgement by unwind after commit");
            }
            if self.drift_publication_identity_after_publish {
                self.reported_publication_identity = DRIFTED_PUBLICATION_ID;
            }
            if self.publish_failure == TestPublishFailure::AfterCommit {
                Err(ZkAmsMkheErrorV1::InvalidAuthentication)
            } else {
                Ok(())
            }
        }

        fn lookup_published_pointer(
            &mut self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1> {
            self.lookup_calls += 1;
            if self.panic_lookup {
                panic!("simulated authoritative lookup panic after commit");
            }
            if self.lookup_error {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            if self.lookup_none {
                return Ok(None);
            }
            let Some(entry) = self.published.iter().find(|entry| entry.pointer == pointer) else {
                return Ok(None);
            };
            let observed_pointer = self.lookup_substitute_pointer.unwrap_or(entry.pointer);
            let mut binding = ZkAmsMkheDirectObjectPublishedBindingV1::new(
                self.publication_identity,
                entry.published_object_identity,
                observed_pointer,
            )?;
            if self.corrupt_lookup_binding {
                binding.binding_digest[0] ^= 1;
            }
            Ok(Some(binding))
        }
    }

    impl ZkAmsMkheDirectObjectReadAtProviderV1 for TestCasStore {
        fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
            Ok(self.provider_identity)
        }

        fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
            if self
                .provider_snapshot_drift_at
                .is_some_and(|call| self.provider_read_calls >= call)
            {
                Ok(DRIFTED_SNAPSHOT_ID)
            } else {
                Ok(self.snapshot_identity)
            }
        }

        fn object_len(
            &mut self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
        ) -> Result<u64, ZkAmsMkheErrorV1> {
            let index = self.published_index(pointer)?;
            let length = u64::try_from(self.published[index].bytes.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if self.provider_len_bias {
                length
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(length)
            }
        }

        fn read_at(
            &mut self,
            pointer: ZkAmsMkheDirectObjectPointerV1,
            absolute_offset: u64,
            destination: &mut [u8],
        ) -> Result<usize, ZkAmsMkheErrorV1> {
            self.provider_read_calls += 1;
            self.provider_read_offsets.push(absolute_offset);
            self.max_provider_read = self.max_provider_read.max(destination.len());
            if self.panic_provider_read_at == Some(self.provider_read_calls) {
                panic!("simulated published-provider read panic");
            }
            let index = self.published_index(pointer)?;
            let start = usize::try_from(absolute_offset)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let copied = if self.provider_short_read_at == Some(self.provider_read_calls) {
                destination.len().saturating_sub(1)
            } else {
                destination.len()
            };
            let end = start
                .checked_add(copied)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            let source = self.published[index]
                .bytes
                .get(start..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            destination[..copied].copy_from_slice(source);
            if self.provider_mutate_read_at == Some(self.provider_read_calls) && copied != 0 {
                destination[copied - 1] ^= 0x40;
            }
            if self.provider_over_read_at == Some(self.provider_read_calls) {
                destination
                    .len()
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
            } else {
                Ok(copied)
            }
        }
    }

    fn publish_test_payload(
        store: &mut TestCasStore,
        kind: ZkAmsMkheDirectObjectKindV1,
        bytes: &[u8],
    ) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1> {
        let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
            kind,
            u64::try_from(bytes.len()).unwrap(),
            store,
        )?;
        for chunk in bytes.chunks(997) {
            transaction.write_exact(chunk)?;
        }
        transaction.finish()
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
    fn cas_publication_seals_rereads_publishes_and_readbacks_exactly() {
        let kind = ZkAmsMkheDirectObjectKindV1::AggregateH1;
        let bytes = payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 19);
        let expected = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();
        let mut store = TestCasStore::new();

        let mut receipt = publish_test_payload(&mut store, kind, &bytes).unwrap();
        receipt.validate().unwrap();
        assert_eq!(receipt.pointer(), expected);
        assert_eq!(receipt.publication_identity(), PUBLICATION_ID);
        assert_ne!(receipt.staging_identity(), PUBLICATION_ID);
        assert_ne!(receipt.seal_identity(), PUBLICATION_ID);
        assert_ne!(receipt.staging_identity(), receipt.seal_identity());
        assert!(!receipt.reconciled_after_publish_error());
        assert_ne!(receipt.receipt_digest(), [0; 32]);

        let binding = receipt.published_binding();
        assert_eq!(binding.publication_identity(), PUBLICATION_ID);
        assert_eq!(binding.pointer(), expected);
        assert_ne!(binding.published_object_identity(), [0; 32]);
        assert_ne!(binding.binding_digest(), [0; 32]);
        let readback = receipt.post_publish_read_receipt();
        assert_eq!(readback.snapshot().pointer(), expected);
        assert_eq!(readback.snapshot().provider_identity(), PROVIDER_ID);
        assert_eq!(readback.snapshot().snapshot_identity(), SNAPSHOT_ID);
        assert_eq!(readback.canonical_bytes(), expected.payload_bytes());
        assert_eq!(readback.payload_blake3(), expected.payload_blake3());

        assert_eq!(store.published.len(), 1);
        assert_eq!(store.published[0].pointer, expected);
        assert_eq!(store.published[0].bytes.as_ref(), bytes.as_slice());
        assert_eq!(store.publish_calls, 1);
        assert_eq!(store.lookup_calls, 1);
        assert_eq!(store.sealed_read_calls, 3);
        assert_eq!(store.provider_read_calls, 3);
        assert!(store.max_write <= ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1);
        assert!(store.max_sealed_read <= ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1);
        assert!(store.max_provider_read <= ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1);

        let mut expected_write_offsets = Vec::new();
        let mut next_offset = 0_u64;
        for chunk in bytes.chunks(997) {
            expected_write_offsets.push(next_offset);
            next_offset += u64::try_from(chunk.len()).unwrap();
        }
        assert_eq!(store.write_offsets, expected_write_offsets);
        assert_eq!(
            store.sealed_read_offsets,
            vec![
                0,
                u64::try_from(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1).unwrap(),
                u64::try_from(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1).unwrap(),
            ]
        );
        assert_eq!(store.provider_read_offsets, store.sealed_read_offsets);
        assert!(!ZK_AMS_MKHE_DIRECT_OBJECT_ADMISSION_GATE_V1);
        assert!(!ZK_AMS_MKHE_DIRECT_OBJECT_RELEASE_GATE_V1);

        receipt.staging_token_digest[0] ^= 1;
        receipt.receipt_digest = direct_object_publication_receipt_digest(&receipt);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn publication_begin_rejects_invalid_bounds_and_zero_session_before_allocation() {
        let kind = ZkAmsMkheDirectObjectKindV1::ProofEnvelope;

        let mut zero_length = TestCasStore::new();
        assert!(
            ZkAmsMkheDirectObjectPublicationTransactionV1::begin(kind, 0, &mut zero_length)
                .is_err()
        );
        assert!(zero_length.staging.is_none());

        let mut oversized = TestCasStore::new();
        assert!(
            ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
                kind,
                kind.payload_ceiling().checked_add(1).unwrap(),
                &mut oversized,
            )
            .is_err()
        );
        assert!(oversized.staging.is_none());

        let mut zero_session = TestCasStore::new();
        zero_session.reported_publication_identity = [0; 32];
        assert!(
            ZkAmsMkheDirectObjectPublicationTransactionV1::begin(kind, 1, &mut zero_session)
                .is_err()
        );
        assert!(zero_session.staging.is_none());
    }

    #[test]
    fn publication_write_failures_and_caught_unwind_permanently_poison_transaction() {
        let bytes = payload(97);
        for failure in 0..4 {
            let mut store = TestCasStore::new();
            match failure {
                0 => store.short_write_at = Some(1),
                1 => store.over_write_at = Some(1),
                2 => store.error_write_at = Some(1),
                3 => store.error_write_after_mutation_at = Some(1),
                _ => unreachable!(),
            }
            let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
                ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                u64::try_from(bytes.len()).unwrap(),
                &mut store,
            )
            .unwrap();
            assert!(transaction.write_exact(&bytes).is_err());
            transaction.publisher.short_write_at = None;
            transaction.publisher.over_write_at = None;
            transaction.publisher.error_write_at = None;
            transaction.publisher.error_write_after_mutation_at = None;
            let calls_after_failure = transaction.publisher.write_calls;
            assert!(transaction.write_exact(&bytes).is_err());
            assert_eq!(transaction.publisher.write_calls, calls_after_failure);
            assert!(transaction.finish().is_err());
            assert!(store.published.is_empty());
            assert_eq!(store.publish_calls, 0);
        }

        for (payload_bytes, invalid_write_bytes) in [
            (4_usize, 0_usize),
            (
                ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1,
                ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1,
            ),
            (3, 4),
        ] {
            let mut store = TestCasStore::new();
            let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
                ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                u64::try_from(payload_bytes).unwrap(),
                &mut store,
            )
            .unwrap();
            let invalid = vec![0x5a; invalid_write_bytes];
            assert!(transaction.write_exact(&invalid).is_err());
            assert_eq!(transaction.publisher.write_calls, 0);
            assert!(transaction.write_exact(&[1]).is_err());
            assert_eq!(transaction.publisher.write_calls, 0);
            assert!(transaction.finish().is_err());
            assert!(store.published.is_empty());
        }

        let mut store = TestCasStore::new();
        store.panic_write_at = Some(1);
        let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            u64::try_from(bytes.len()).unwrap(),
            &mut store,
        )
        .unwrap();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = transaction.write_exact(&bytes);
        }));
        assert!(unwind.is_err());
        transaction.publisher.panic_write_at = None;
        assert_eq!(transaction.publisher.write_calls, 1);
        assert!(transaction.write_exact(&bytes).is_err());
        assert_eq!(transaction.publisher.write_calls, 1);
        assert!(transaction.finish().is_err());
        assert!(store.published.is_empty());
        assert_eq!(store.publish_calls, 0);
    }

    #[test]
    fn sealed_stage_length_token_and_reread_attacks_never_publish() {
        let bytes = payload(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 37);
        for attack in 0..9 {
            let mut store = TestCasStore::new();
            match attack {
                0 => store.staged_len_bias = true,
                1 => store.corrupt_staging_token = true,
                2 => store.sealed_len_bias = true,
                3 => store.short_sealed_read_at = Some(1),
                4 => store.over_sealed_read_at = Some(1),
                5 => store.mutate_sealed_read_at = Some(1),
                6 => store.corrupt_seal_lineage = true,
                7 => store.alias_seal_identity = true,
                8 => store.mutate_write_at = Some(1),
                _ => unreachable!(),
            }
            assert!(
                publish_test_payload(
                    &mut store,
                    ZkAmsMkheDirectObjectKindV1::RkgNormalization,
                    &bytes,
                )
                .is_err(),
                "accepted sealed-stage attack {attack}"
            );
            assert!(
                store.published.is_empty(),
                "published after sealed-stage attack {attack}"
            );
        }

        let mut panicking_store = TestCasStore::new();
        panicking_store.panic_sealed_read_at = Some(1);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = publish_test_payload(
                &mut panicking_store,
                ZkAmsMkheDirectObjectKindV1::RkgNormalization,
                &bytes,
            );
        }));
        assert!(unwind.is_err());
        assert!(panicking_store.staging.is_none());
        assert!(panicking_store.sealed.is_some());
        assert!(panicking_store.published.is_empty());
        assert_eq!(panicking_store.publish_calls, 0);
    }

    #[test]
    fn ambiguous_publish_ack_reconciles_only_by_exact_authoritative_lookup() {
        let kind = ZkAmsMkheDirectObjectKindV1::CpkPartyB;
        let bytes = payload(4097);
        let expected = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();

        let mut lost_ack = TestCasStore::new();
        lost_ack.publish_failure = TestPublishFailure::AfterCommit;
        let receipt = publish_test_payload(&mut lost_ack, kind, &bytes).unwrap();
        assert_eq!(receipt.pointer(), expected);
        assert!(receipt.reconciled_after_publish_error());
        assert_eq!(lost_ack.published.len(), 1);
        assert_eq!(lost_ack.publish_calls, 1);
        assert_eq!(lost_ack.lookup_calls, 1);

        let mut rejected_before_commit = TestCasStore::new();
        rejected_before_commit.publish_failure = TestPublishFailure::BeforeCommit;
        assert!(publish_test_payload(&mut rejected_before_commit, kind, &bytes).is_err());
        assert!(rejected_before_commit.published.is_empty());
        assert_eq!(rejected_before_commit.publish_calls, 1);
        assert_eq!(rejected_before_commit.lookup_calls, 1);

        let mut absent_lookup = TestCasStore::new();
        absent_lookup.lookup_none = true;
        assert!(publish_test_payload(&mut absent_lookup, kind, &bytes).is_err());
        assert_eq!(absent_lookup.published.len(), 1);
        assert_eq!(absent_lookup.lookup_calls, 1);

        let mut failed_lookup = TestCasStore::new();
        failed_lookup.lookup_error = true;
        assert!(publish_test_payload(&mut failed_lookup, kind, &bytes).is_err());
        assert_eq!(failed_lookup.published.len(), 1);
        assert_eq!(failed_lookup.lookup_calls, 1);
    }

    #[test]
    fn caught_publish_unwind_cannot_erase_commit_and_retry_is_idempotent() {
        let kind = ZkAmsMkheDirectObjectKindV1::GaloisB;
        let bytes = payload(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 29);
        let expected = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();
        let mut store = TestCasStore::new();
        store.panic_publish_after_commit = true;

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = publish_test_payload(&mut store, kind, &bytes);
        }));
        assert!(unwind.is_err());
        assert_eq!(store.published.len(), 1);
        assert_eq!(store.published[0].pointer, expected);
        assert_eq!(store.published[0].bytes.as_ref(), bytes.as_slice());
        assert_eq!(store.lookup_calls, 0);
        let published_object_identity = store.published[0].published_object_identity;

        store.panic_publish_after_commit = false;
        let receipt = publish_test_payload(&mut store, kind, &bytes).unwrap();
        assert_eq!(receipt.pointer(), expected);
        assert_eq!(store.published.len(), 1);
        assert_eq!(
            store.published[0].published_object_identity,
            published_object_identity
        );
        assert_eq!(store.publish_calls, 2);
        assert_eq!(store.lookup_calls, 1);
    }

    #[test]
    fn caught_lookup_and_readback_unwinds_preserve_commit_for_idempotent_retry() {
        let kind = ZkAmsMkheDirectObjectKindV1::AggregateH0;
        let bytes = payload(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 41);
        let expected = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();

        for panic_site in 0..2 {
            let mut store = TestCasStore::new();
            if panic_site == 0 {
                store.panic_lookup = true;
            } else {
                store.panic_provider_read_at = Some(1);
            }
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = publish_test_payload(&mut store, kind, &bytes);
            }));
            assert!(unwind.is_err());
            assert_eq!(store.published.len(), 1);
            assert_eq!(store.published[0].pointer, expected);
            assert_eq!(store.published[0].bytes.as_ref(), bytes.as_slice());
            let published_object_identity = store.published[0].published_object_identity;

            store.panic_lookup = false;
            store.panic_provider_read_at = None;
            let receipt = publish_test_payload(&mut store, kind, &bytes).unwrap();
            assert_eq!(receipt.pointer(), expected);
            assert_eq!(store.published.len(), 1);
            assert_eq!(
                store.published[0].published_object_identity,
                published_object_identity
            );
            assert_eq!(store.publish_calls, 2);
        }
    }

    #[test]
    fn occupied_pointer_with_different_bytes_is_never_overwritten_or_receipted() {
        let kind = ZkAmsMkheDirectObjectKindV1::ProofEnvelope;
        let bytes = payload(4099);
        let pointer = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();
        let mut conflicting_bytes = bytes.clone();
        conflicting_bytes[2048] ^= 0x80;
        let conflicting_bytes: Arc<[u8]> = Arc::from(conflicting_bytes);
        let mut store = TestCasStore::new();
        store.published.push(TestPublishedObject {
            pointer,
            published_object_identity: [0xb1; 32],
            bytes: Arc::clone(&conflicting_bytes),
        });

        assert!(publish_test_payload(&mut store, kind, &bytes).is_err());
        assert_eq!(store.publish_calls, 1);
        assert_eq!(store.lookup_calls, 1);
        assert_eq!(store.published.len(), 1);
        assert_eq!(
            store.published[0].bytes.as_ref(),
            conflicting_bytes.as_ref()
        );
        assert_ne!(store.published[0].bytes.as_ref(), bytes.as_slice());
    }

    #[test]
    fn post_publish_readback_failure_leaves_immutable_object_and_retry_is_idempotent() {
        let kind = ZkAmsMkheDirectObjectKindV1::CpkRelationProof;
        let bytes = payload(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 73);
        let expected = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes).unwrap();

        for fault in 0..4 {
            let mut store = TestCasStore::new();
            match fault {
                0 => store.provider_short_read_at = Some(1),
                1 => store.provider_over_read_at = Some(1),
                2 => store.provider_mutate_read_at = Some(1),
                3 => store.provider_len_bias = true,
                _ => unreachable!(),
            }
            assert!(publish_test_payload(&mut store, kind, &bytes).is_err());
            assert_eq!(store.published.len(), 1);
            assert_eq!(store.published[0].pointer, expected);
            assert_eq!(store.published[0].bytes.as_ref(), bytes.as_slice());
            let published_object_identity = store.published[0].published_object_identity;

            store.provider_short_read_at = None;
            store.provider_over_read_at = None;
            store.provider_mutate_read_at = None;
            store.provider_len_bias = false;
            let receipt = publish_test_payload(&mut store, kind, &bytes).unwrap();
            assert_eq!(receipt.pointer(), expected);
            assert!(!receipt.reconciled_after_publish_error());
            assert_eq!(store.published.len(), 1);
            assert_eq!(
                store.published[0].published_object_identity,
                published_object_identity
            );
            assert_eq!(store.publish_calls, 2);
            assert_eq!(store.lookup_calls, 2);
        }
    }

    #[test]
    fn lookup_substitution_identity_drift_and_provider_snapshot_drift_fail_closed() {
        let kind = ZkAmsMkheDirectObjectKindV1::RkgK;
        let bytes = payload(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 11);
        let substitute = ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &payload(31)).unwrap();

        for lookup_attack in 0..2 {
            let mut store = TestCasStore::new();
            if lookup_attack == 0 {
                store.lookup_substitute_pointer = Some(substitute);
            } else {
                store.corrupt_lookup_binding = true;
            }
            assert!(publish_test_payload(&mut store, kind, &bytes).is_err());
            assert_eq!(store.published.len(), 1);
            assert_eq!(store.lookup_calls, 1);
        }

        for drift_phase in 0..2 {
            let mut store = TestCasStore::new();
            if drift_phase == 0 {
                store.drift_publication_identity_after_write = true;
            } else {
                store.drift_publication_identity_after_seal = true;
            }
            assert!(publish_test_payload(&mut store, kind, &bytes).is_err());
            assert!(store.published.is_empty());
            assert_eq!(store.publish_calls, 0);
            assert_eq!(store.reported_publication_identity, DRIFTED_PUBLICATION_ID);
        }

        let mut identity_drift = TestCasStore::new();
        identity_drift.drift_publication_identity_after_publish = true;
        assert!(publish_test_payload(&mut identity_drift, kind, &bytes).is_err());
        assert_eq!(identity_drift.published.len(), 1);
        assert_eq!(identity_drift.lookup_calls, 1);
        assert_eq!(
            identity_drift.reported_publication_identity,
            DRIFTED_PUBLICATION_ID
        );

        let mut snapshot_drift = TestCasStore::new();
        snapshot_drift.provider_snapshot_drift_at = Some(1);
        assert!(publish_test_payload(&mut snapshot_drift, kind, &bytes).is_err());
        assert_eq!(snapshot_drift.published.len(), 1);
        assert_eq!(snapshot_drift.provider_read_calls, 1);
    }

    #[test]
    fn dropping_incomplete_transaction_cannot_publish_or_unpublish() {
        let mut store = TestCasStore::new();
        {
            let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
                ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
                97,
                &mut store,
            )
            .unwrap();
            transaction.write_exact(&payload(31)).unwrap();
            assert_eq!(transaction.remaining_bytes(), 66);
        }
        assert_eq!(store.publish_calls, 0);
        assert_eq!(store.lookup_calls, 0);
        assert!(store.published.is_empty());
        assert!(store.sealed.is_none());
        assert_eq!(store.staging.as_ref().unwrap().bytes.len(), 31);
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
