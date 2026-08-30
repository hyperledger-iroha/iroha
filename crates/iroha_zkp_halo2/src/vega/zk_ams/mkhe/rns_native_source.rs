//! Dependency-neutral confidential-source boundary for the replacement
//! RNS-native MKHE proof.
//!
//! The ZKP crate deliberately does not depend on Iroha's filesystem or AEAD
//! implementation.  Instead, proof orchestration accepts this narrow provider
//! interface.  `iroha_core` implements it with the already-locked
//! `iroha_crypto::confidential_spool` backend.  The resulting receipt is only
//! structural provenance: it is neither a hiding commitment nor release
//! authority.

use super::rns_native_profile::{
    ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, zk_ams_mkhe_rns_native_profile_v1,
    zk_ams_mkhe_rns_native_topology_v1,
};
use crate::vega::sponge::Keccak256;

const SOURCE_BINDING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-binding";
const SOURCE_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-context";
const SOURCE_RECEIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-receipt";
const SOURCE_REPEATABILITY_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-repeatability-evidence";

/// Source-boundary schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1: u8 = 1;
/// Plaintext bytes in one canonical source or encryption-witness block.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1: u64 = 8_192;
/// Plaintext bytes in one fresh-encryption nonce record.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1: u64 = 32;
/// Canonical source blocks per committed opening.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1: u64 = 896;
/// Exact write-once slots in the main confidential arena.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1: u64 = 38_528;
/// Exact write-once slots in the nonce confidential arena.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1: u64 = 43;
/// Exact encrypted bytes in the main arena, including one 16-byte AEAD tag per slot.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_FILE_BYTES_V1: u64 = 316_237_824;
/// Exact encrypted bytes in the nonce arena, including one 16-byte AEAD tag per slot.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_FILE_BYTES_V1: u64 = 2_064;
/// Exact combined encrypted source bytes.
pub const ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1: u64 = 316_239_888;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 == 43);
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1
            == ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as u64
                * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1
    );
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1
            == ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 as u64
    );
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_FILE_BYTES_V1
            == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1
                * (ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 + 16)
    );
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_FILE_BYTES_V1
            == ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1
                * (ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1 + 16)
    );
};

/// One of the two fixed confidential arenas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheRnsNativeSourceArenaV1 {
    /// Canonical source values and three signed encryption witnesses.
    Main = 1,
    /// Fresh-encryption nonces.
    Nonce = 2,
}

impl ZkAmsMkheRnsNativeSourceArenaV1 {
    /// Return the exact write-once slot count.
    #[must_use]
    pub const fn slot_count(self) -> u64 {
        match self {
            Self::Main => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1,
            Self::Nonce => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1,
        }
    }

    /// Return the exact plaintext bytes in every slot.
    #[must_use]
    pub const fn plaintext_bytes(self) -> u64 {
        match self {
            Self::Main => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1,
            Self::Nonce => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1,
        }
    }

    /// Return the exact encrypted file bytes, including AEAD tags.
    #[must_use]
    pub const fn file_bytes(self) -> u64 {
        match self {
            Self::Main => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_FILE_BYTES_V1,
            Self::Nonce => ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_FILE_BYTES_V1,
        }
    }
}

/// Coarse, path-free failure returned across the confidential-source boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheRnsNativeSourceErrorV1 {
    /// A release, profile, topology, statement, or operational digest is invalid.
    InvalidContext,
    /// The provider received anything other than the sole canonical layout.
    InvalidLayout,
    /// A public source or process resource ceiling was exceeded.
    ResourceCeilingExceeded,
    /// A bounded secret allocation failed.
    Allocation,
    /// The platform cannot provide the required confidential backend.
    BackendUnavailable,
    /// A filesystem operation failed; paths and platform messages are withheld.
    Storage,
    /// A write did not target the exact next slot or used the wrong arena/chunk.
    UnexpectedWrite,
    /// Sealing was requested before the exact fixed layout was complete.
    Incomplete,
    /// An encrypted record failed authentication or context binding.
    Authentication,
    /// A prior operational failure permanently poisoned the handle.
    Poisoned,
}

impl core::fmt::Display for ZkAmsMkheRnsNativeSourceErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContext => "invalid RNS-native confidential-source context",
            Self::InvalidLayout => "invalid RNS-native confidential-source layout",
            Self::ResourceCeilingExceeded => "RNS-native confidential-source ceiling exceeded",
            Self::Allocation => "RNS-native confidential-source allocation failed",
            Self::BackendUnavailable => "RNS-native confidential-source backend unavailable",
            Self::Storage => "RNS-native confidential-source storage operation failed",
            Self::UnexpectedWrite => "unexpected RNS-native confidential-source write",
            Self::Incomplete => "incomplete RNS-native confidential source",
            Self::Authentication => "RNS-native confidential-source authentication failed",
            Self::Poisoned => "RNS-native confidential-source handle poisoned",
        })
    }
}

impl std::error::Error for ZkAmsMkheRnsNativeSourceErrorV1 {}

/// Exact non-secret context and geometry for the two confidential arenas.
///
/// Fields are private so a provider can only receive layouts constructed and
/// validated by this module.  This value carries no release authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeSourceLayoutV1 {
    version: u8,
    profile_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    main_context_digest: [u8; 32],
    nonce_context_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeSourceLayoutV1 {
    /// Construct the sole canonical source layout.
    ///
    /// # Errors
    ///
    /// Rejects zero, duplicated, or non-canonical profile/topology identities.
    pub fn new(
        profile_digest: [u8; 32],
        topology_digest: [u8; 32],
        release_candidate_digest: [u8; 32],
        statement_digest: [u8; 32],
        operational_context_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        Self::from_inputs(
            profile_digest,
            topology_digest,
            release_candidate_digest,
            statement_digest,
            operational_context_digest,
        )
    }

    /// Revalidate every fixed identity and derived digest.
    pub fn validate(self) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
        let rebuilt = Self::from_inputs(
            self.profile_digest,
            self.topology_digest,
            self.release_candidate_digest,
            self.statement_digest,
            self.operational_context_digest,
        )?;
        if self != rebuilt {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidLayout);
        }
        Ok(())
    }

    fn from_inputs(
        profile_digest: [u8; 32],
        topology_digest: [u8; 32],
        release_candidate_digest: [u8; 32],
        statement_digest: [u8; 32],
        operational_context_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        let expected_profile = zk_ams_mkhe_rns_native_profile_v1()
            .map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)?;
        let expected_topology = zk_ams_mkhe_rns_native_topology_v1()
            .map_err(|_| ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)?;
        let identities = [
            profile_digest,
            topology_digest,
            release_candidate_digest,
            statement_digest,
            operational_context_digest,
        ];
        if identities
            .iter()
            .any(|digest| digest.iter().all(|byte| *byte == 0))
            || !digests_are_distinct_v1(&identities)
            || profile_digest != expected_profile.profile_digest
            || topology_digest != expected_topology.topology_digest
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext);
        }
        let source_binding_digest = source_binding_digest_v1(
            profile_digest,
            topology_digest,
            release_candidate_digest,
            statement_digest,
            operational_context_digest,
        );
        Ok(Self {
            version: ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1,
            profile_digest,
            topology_digest,
            release_candidate_digest,
            statement_digest,
            operational_context_digest,
            source_binding_digest,
            main_context_digest: arena_context_digest_v1(
                source_binding_digest,
                ZkAmsMkheRnsNativeSourceArenaV1::Main,
            ),
            nonce_context_digest: arena_context_digest_v1(
                source_binding_digest,
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
            ),
        })
    }

    /// Return the exact profile identity.
    #[must_use]
    pub const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }

    /// Return the exact topology identity.
    #[must_use]
    pub const fn topology_digest(self) -> [u8; 32] {
        self.topology_digest
    }

    /// Return the exact non-authorizing replacement release-candidate identity.
    #[must_use]
    pub const fn release_candidate_digest(self) -> [u8; 32] {
        self.release_candidate_digest
    }

    /// Return the exact proof-statement identity.
    #[must_use]
    pub const fn statement_digest(self) -> [u8; 32] {
        self.statement_digest
    }

    /// Return the exact operational/replay context identity.
    #[must_use]
    pub const fn operational_context_digest(self) -> [u8; 32] {
        self.operational_context_digest
    }

    /// Return the digest binding every identity and fixed source geometry.
    #[must_use]
    pub const fn source_binding_digest(self) -> [u8; 32] {
        self.source_binding_digest
    }

    /// Return the AEAD context digest for one fixed arena.
    #[must_use]
    pub const fn arena_context_digest(self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self.main_context_digest,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self.nonce_context_digest,
        }
    }
}

fn digests_are_distinct_v1(digests: &[[u8; 32]]) -> bool {
    digests
        .iter()
        .enumerate()
        .all(|(index, digest)| !digests[index + 1..].contains(digest))
}

fn source_binding_digest_v1(
    profile_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_BINDING_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1]);
    hash.update(&profile_digest);
    hash.update(&topology_digest);
    hash.update(&release_candidate_digest);
    hash.update(&statement_digest);
    hash.update(&operational_context_digest);
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1.to_be_bytes());
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1.to_be_bytes());
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1.to_be_bytes());
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_PLAINTEXT_BYTES_V1.to_be_bytes());
    hash.finalize()
}

fn arena_context_digest_v1(
    source_binding_digest: [u8; 32],
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_CONTEXT_DOMAIN_V1);
    hash.update(&source_binding_digest);
    hash.update(&[arena as u8]);
    hash.update(&arena.slot_count().to_be_bytes());
    hash.update(&arena.plaintext_bytes().to_be_bytes());
    hash.finalize()
}

/// Move-only secret chunk supplied by a concrete confidential backend.
pub trait ZkAmsMkheRnsNativeSecretChunkV1 {
    /// Arena whose exact record width this allocation satisfies.
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1;
    /// Borrow the exact secret bytes.
    fn as_slice(&self) -> &[u8];
    /// Mutably borrow the exact secret bytes for in-place construction.
    fn as_mut_slice(&mut self) -> &mut [u8];
}

/// Move-only immutable source snapshot.
pub trait ZkAmsMkheRnsNativeSourceSnapshotV1 {
    /// Concrete secret owner returned by authenticated reads.
    type Chunk: ZkAmsMkheRnsNativeSecretChunkV1;

    /// Return the exact validated layout retained by this snapshot.
    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1;
    /// Return the backend's non-authorizing encrypted-snapshot digest.
    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32];
    /// Authenticate and read one slot into a move-only secret owner.
    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1>;

    /// Derive the non-authorizing structural receipt for this live snapshot.
    fn structural_receipt(
        &self,
    ) -> Result<ZkAmsMkheRnsNativeSourceReceiptV1, ZkAmsMkheRnsNativeSourceErrorV1> {
        ZkAmsMkheRnsNativeSourceReceiptV1::new(
            self.layout(),
            self.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main),
            self.snapshot_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce),
        )
    }
}

/// Move-only immutable source snapshot that supports authenticated repeated
/// random reads for the lifetime of the owner.
///
/// This is deliberately a distinct capability rather than a blanket
/// implementation for [`ZkAmsMkheRnsNativeSourceSnapshotV1`].  Implementors
/// must return the same authenticated plaintext for the same arena/slot in any
/// order, retain the same layout and structural receipt across successful
/// reads, and permanently poison their backing owner after an operational or
/// authentication failure.  Concrete implementations must not implement
/// `Clone` or `Copy`.
///
/// Implementations must be backed by executable conformance evidence.  The
/// Core confidential-spool adapter implements this capability after exercising
/// repeated same-slot reads, descending-order reads, immutable snapshot
/// digests, pure preflight failures, and poison-on-operational-failure behavior.
/// This marker and its evidence remain non-authorizing.
pub trait ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1: ZkAmsMkheRnsNativeSourceSnapshotV1 {}

/// Deterministic, plaintext-free evidence from the fixed repeatability probe.
///
/// The runner compares authenticated chunks only while their move-only owners
/// are live.  Neither plaintext bytes nor a digest derived from plaintext is
/// retained in this record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeRepeatableSourceEvidenceV1 {
    version: u8,
    source_binding_digest: [u8; 32],
    structural_receipt_digest: [u8; 32],
    main_snapshot_digest: [u8; 32],
    nonce_snapshot_digest: [u8; 32],
    authenticated_read_count: u8,
    same_slot_main_equal: bool,
    same_slot_nonce_equal: bool,
    descending_order_complete: bool,
    structural_receipt_unchanged: bool,
    evidence_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeRepeatableSourceEvidenceV1 {
    /// Validate the fixed probe shape and every public binding.
    pub fn validate(
        self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
        layout.validate()?;
        receipt.validate(layout)?;
        if self.version != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1
            || self.source_binding_digest != layout.source_binding_digest()
            || self.structural_receipt_digest != receipt.receipt_digest
            || self.main_snapshot_digest != receipt.main_snapshot_digest
            || self.nonce_snapshot_digest != receipt.nonce_snapshot_digest
            || self.authenticated_read_count != 6
            || !self.same_slot_main_equal
            || !self.same_slot_nonce_equal
            || !self.descending_order_complete
            || !self.structural_receipt_unchanged
            || self.evidence_digest == [0; 32]
            || self.evidence_digest != repeatability_evidence_digest_v1(self)
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
        }
        Ok(())
    }

    /// Digest of the plaintext-free conformance record.
    #[must_use]
    pub const fn evidence_digest(self) -> [u8; 32] {
        self.evidence_digest
    }
}

/// Execute the canonical authenticated repeatability probe.
///
/// Reads intentionally begin at each arena's last slot and then move to slot
/// zero before repeating slot zero.  Successful reads must leave the exact
/// layout and structural receipt unchanged.  Secret chunks are compared in
/// place and immediately dropped; no secret-derived digest or detached buffer
/// is produced.
pub fn verify_zk_ams_mkhe_rns_native_source_repeatability_v1<S>(
    snapshot: &mut S,
) -> Result<ZkAmsMkheRnsNativeRepeatableSourceEvidenceV1, ZkAmsMkheRnsNativeSourceErrorV1>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1 + ?Sized,
{
    let layout_before = snapshot.layout();
    layout_before.validate()?;
    let receipt_before = snapshot.structural_receipt()?;
    receipt_before.validate(layout_before)?;

    let last_main = snapshot.read_slot(
        ZkAmsMkheRnsNativeSourceArenaV1::Main,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1 - 1,
    )?;
    validate_secret_chunk_v1(&last_main, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
    drop(last_main);
    let main_first = snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)?;
    validate_secret_chunk_v1(&main_first, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
    let main_repeat = snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)?;
    validate_secret_chunk_v1(&main_repeat, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
    let same_slot_main_equal = secret_chunks_equal_v1(&main_first, &main_repeat);
    drop((main_first, main_repeat));

    let last_nonce = snapshot.read_slot(
        ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1 - 1,
    )?;
    validate_secret_chunk_v1(&last_nonce, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
    drop(last_nonce);
    let nonce_first = snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0)?;
    validate_secret_chunk_v1(&nonce_first, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
    let nonce_repeat = snapshot.read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0)?;
    validate_secret_chunk_v1(&nonce_repeat, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
    let same_slot_nonce_equal = secret_chunks_equal_v1(&nonce_first, &nonce_repeat);
    drop((nonce_first, nonce_repeat));

    let layout_after = snapshot.layout();
    let receipt_after = snapshot.structural_receipt()?;
    let structural_receipt_unchanged =
        layout_after == layout_before && receipt_after == receipt_before;
    if !same_slot_main_equal || !same_slot_nonce_equal || !structural_receipt_unchanged {
        return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
    }
    let mut evidence = ZkAmsMkheRnsNativeRepeatableSourceEvidenceV1 {
        version: ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1,
        source_binding_digest: layout_before.source_binding_digest(),
        structural_receipt_digest: receipt_before.receipt_digest,
        main_snapshot_digest: receipt_before.main_snapshot_digest,
        nonce_snapshot_digest: receipt_before.nonce_snapshot_digest,
        authenticated_read_count: 6,
        same_slot_main_equal,
        same_slot_nonce_equal,
        descending_order_complete: true,
        structural_receipt_unchanged,
        evidence_digest: [0; 32],
    };
    evidence.evidence_digest = repeatability_evidence_digest_v1(evidence);
    evidence.validate(layout_before, receipt_before)?;
    Ok(evidence)
}

fn validate_secret_chunk_v1<C: ZkAmsMkheRnsNativeSecretChunkV1 + ?Sized>(
    chunk: &C,
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
    if chunk.arena() != arena || chunk.as_slice().len() as u64 != arena.plaintext_bytes() {
        return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
    }
    Ok(())
}

fn secret_chunks_equal_v1<C: ZkAmsMkheRnsNativeSecretChunkV1 + ?Sized>(
    left: &C,
    right: &C,
) -> bool {
    if left.arena() != right.arena() || left.as_slice().len() != right.as_slice().len() {
        return false;
    }
    left.as_slice()
        .iter()
        .zip(right.as_slice())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn repeatability_evidence_digest_v1(
    evidence: ZkAmsMkheRnsNativeRepeatableSourceEvidenceV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_REPEATABILITY_EVIDENCE_DOMAIN_V1);
    hash.update(&[evidence.version]);
    hash.update(&evidence.source_binding_digest);
    hash.update(&evidence.structural_receipt_digest);
    hash.update(&evidence.main_snapshot_digest);
    hash.update(&evidence.nonce_snapshot_digest);
    hash.update(&[
        evidence.authenticated_read_count,
        evidence.same_slot_main_equal.into(),
        evidence.same_slot_nonce_equal.into(),
        evidence.descending_order_complete.into(),
        evidence.structural_receipt_unchanged.into(),
    ]);
    hash.finalize()
}

/// Move-only sequential source writer.
pub trait ZkAmsMkheRnsNativeSourceWriterV1: Sized {
    /// Concrete secret owner accepted by writes.
    type Chunk: ZkAmsMkheRnsNativeSecretChunkV1;
    /// Immutable snapshot returned only after every slot is present.
    type Snapshot: ZkAmsMkheRnsNativeSourceSnapshotV1<Chunk = Self::Chunk>;

    /// Return the exact validated layout retained by this writer.
    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1;

    /// Allocate one exact-size, initially zeroed secret record.
    fn allocate_chunk(
        &self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1>;
    /// Consume and write one record at the exact next slot of its arena.
    fn write_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
        chunk: Self::Chunk,
    ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1>;
    /// Consume the writer and authenticate both complete arenas.
    fn seal(self) -> Result<Self::Snapshot, ZkAmsMkheRnsNativeSourceErrorV1>;
}

/// Injected factory for a real confidential source backend.
pub trait ZkAmsMkheRnsNativeSourceProviderV1 {
    /// Concrete move-only writer created by this provider.
    type Writer: ZkAmsMkheRnsNativeSourceWriterV1;

    /// Create both fixed arenas for one validated source layout.
    fn create(
        &self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    ) -> Result<Self::Writer, ZkAmsMkheRnsNativeSourceErrorV1>;
}

/// Non-authorizing structural provenance of two authenticated source arenas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeSourceReceiptV1 {
    /// Source identity and fixed geometry digest.
    pub source_binding_digest: [u8; 32],
    /// Backend digest of the complete encrypted main arena.
    pub main_snapshot_digest: [u8; 32],
    /// Backend digest of the complete encrypted nonce arena.
    pub nonce_snapshot_digest: [u8; 32],
    /// Digest of the preceding receipt fields.
    pub receipt_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeSourceReceiptV1 {
    fn new(
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        main_snapshot_digest: [u8; 32],
        nonce_snapshot_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        layout.validate()?;
        if main_snapshot_digest.iter().all(|byte| *byte == 0)
            || nonce_snapshot_digest.iter().all(|byte| *byte == 0)
            || main_snapshot_digest == nonce_snapshot_digest
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
        }
        let mut receipt = Self {
            source_binding_digest: layout.source_binding_digest(),
            main_snapshot_digest,
            nonce_snapshot_digest,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = receipt_digest_v1(receipt);
        Ok(receipt)
    }

    /// Revalidate the structural receipt against the exact live layout.
    pub fn validate(
        self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
        layout.validate()?;
        if self.source_binding_digest != layout.source_binding_digest()
            || self.main_snapshot_digest.iter().all(|byte| *byte == 0)
            || self.nonce_snapshot_digest.iter().all(|byte| *byte == 0)
            || self.main_snapshot_digest == self.nonce_snapshot_digest
            || self.receipt_digest == [0; 32]
            || self.receipt_digest != receipt_digest_v1(self)
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
        }
        Ok(())
    }
}

fn receipt_digest_v1(receipt: ZkAmsMkheRnsNativeSourceReceiptV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_RECEIPT_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_SOURCE_VERSION_V1]);
    hash.update(&receipt.source_binding_digest);
    hash.update(&receipt.main_snapshot_digest);
    hash.update(&receipt.nonce_snapshot_digest);
    hash.finalize()
}

#[cfg(test)]
#[path = "rns_native_source_tests.rs"]
mod tests;
