//! Fail-closed confidential-source topology for Phase-23 RNS-Link.
//!
//! This module freezes the secret-only store geometry and its move-only state
//! transitions.  The public collective key and ciphertext limbs stay in their
//! existing authenticated direct-object store; this source reserves space only
//! for the 43 canonical plaintexts, their three signed encryption witnesses,
//! and their 43 fresh-encryption nonces.
//!
//! A private adapter owns the concrete confidential-spool writers and immutable
//! snapshots.  Construction consumes an already validated Phase-23 context;
//! blocks are accepted only in the frozen record/component/block order, with a
//! nonce completing each record.  The adapter exposes no path, key, codec, raw
//! snapshot bytes, or detached-digest constructor.  Its receipt is still only
//! structural metadata, never a hiding commitment, algebraic proof, or release
//! receipt.
//!
//! The collective-encryption orchestrator does not yet feed this writer.  Thus
//! `confidential_backend_wired` means only that the real backend is reachable
//! from a validated context; every stronger construction and release axis
//! remains closed.
//! The adapter also inherits the leaf's explicit exclusions for secure
//! deletion, swap/core/page-cache control, panic-abort erasure, and measured RSS.
use std::path::Path;
use iroha_confidential_spool::ConfidentialSpoolChunkV1;
use crate::vega::sponge::Keccak256;
use super::super::ZkAmsMkheErrorV1;
use super::{
    RNS_LINK_FAMILY_ORDER_V1, RNS_LINK_RELEASE_COMMITMENTS_V1, RNS_LINK_VERSION_V1,
    ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1, ZkAmsPhase23RnsLinkContextV1,
    ZkAmsPhase23RnsLinkFamilyV1, ZkAmsPhase23RnsLinkReleaseGeometryV1,
    derive_zk_ams_phase23_rns_link_release_geometry_v1,
};
#[path = "phase23_rns_link_external_spool.rs"]
mod confidential_spool;
use confidential_spool::{RnsLinkSecretSpoolSnapshotsV1, RnsLinkSecretSpoolWriterV1};
const SOURCE_VERSION_V1: u8 = 1;
const SOURCE_RECORD_COUNT_V1: u16 = RNS_LINK_RELEASE_COMMITMENTS_V1 as u16;
const SOURCE_EQUATIONS_PER_RECORD_V1: u16 = 2;
const SOURCE_RELATION_COORDINATE_COUNT_V1: u32 = SOURCE_RECORD_COUNT_V1 as u32
    * SOURCE_EQUATIONS_PER_RECORD_V1 as u32
    * ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 as u32;
const SECRET_MAIN_PLAINTEXT_BYTES_V1: u64 = 8_192;
const SECRET_NONCE_PLAINTEXT_BYTES_V1: u64 = 32;
const SECRET_AEAD_TAG_BYTES_V1: u64 = 16;
const SECRET_MAIN_RECORD_BYTES_V1: u64 = SECRET_MAIN_PLAINTEXT_BYTES_V1 + SECRET_AEAD_TAG_BYTES_V1;
const SECRET_NONCE_RECORD_BYTES_V1: u64 =
    SECRET_NONCE_PLAINTEXT_BYTES_V1 + SECRET_AEAD_TAG_BYTES_V1;
const CANONICAL_COEFFICIENT_BYTES_V1: u64 = 32;
const SIGNED_COEFFICIENT_BYTES_V1: u64 = 8;
const CANONICAL_BLOCK_ENCODING_TAG_V1: &[u8] = b"canonical-plaintext:coefficient:big-endian";
const SIGNED_BLOCK_ENCODING_TAG_V1: &[u8] =
    b"encryption-witness-r-e0-e1:i64:twos-complement:big-endian";
const NONCE_ENCODING_TAG_V1: &[u8] = b"fresh-encryption-nonce:raw-bytes";
const CANONICAL_COEFFICIENTS_PER_BLOCK_V1: u16 =
    (SECRET_MAIN_PLAINTEXT_BYTES_V1 / CANONICAL_COEFFICIENT_BYTES_V1) as u16;
const SIGNED_COEFFICIENTS_PER_BLOCK_V1: u16 =
    (SECRET_MAIN_PLAINTEXT_BYTES_V1 / SIGNED_COEFFICIENT_BYTES_V1) as u16;
const RING_DEGREE_V1: u32 = 131_072;
const FULL_PACKED_USED_SLOTS_V1: u32 = 65_536;
const CANONICAL_BLOCKS_PER_RECORD_V1: u16 =
    (RING_DEGREE_V1 / CANONICAL_COEFFICIENTS_PER_BLOCK_V1 as u32) as u16;
const SIGNED_BLOCKS_PER_POLYNOMIAL_V1: u16 =
    (RING_DEGREE_V1 / SIGNED_COEFFICIENTS_PER_BLOCK_V1 as u32) as u16;
const SIGNED_POLYNOMIALS_PER_RECORD_V1: u16 = 3;
const SECRET_MAIN_BLOCKS_PER_RECORD_V1: u16 = CANONICAL_BLOCKS_PER_RECORD_V1
    + SIGNED_POLYNOMIALS_PER_RECORD_V1 * SIGNED_BLOCKS_PER_POLYNOMIAL_V1;
const SECRET_MAIN_SLOT_COUNT_V1: u64 =
    SOURCE_RECORD_COUNT_V1 as u64 * SECRET_MAIN_BLOCKS_PER_RECORD_V1 as u64;
const SECRET_NONCE_SLOT_COUNT_V1: u64 = SOURCE_RECORD_COUNT_V1 as u64;
const SECRET_MAIN_FILE_BYTES_V1: u64 = SECRET_MAIN_SLOT_COUNT_V1 * SECRET_MAIN_RECORD_BYTES_V1;
const SECRET_NONCE_FILE_BYTES_V1: u64 = SECRET_NONCE_SLOT_COUNT_V1 * SECRET_NONCE_RECORD_BYTES_V1;
const SECRET_TOTAL_FILE_BYTES_V1: u64 = SECRET_MAIN_FILE_BYTES_V1 + SECRET_NONCE_FILE_BYTES_V1;
const SOURCE_MAPPING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.secret-source-mapping";
const SOURCE_ABSOLUTE_MAIN_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-main-mapping";
const SOURCE_ABSOLUTE_NONCE_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-nonce-mapping";
const SOURCE_ABSOLUTE_RELATION_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-absolute-relation-mapping";
const SOURCE_MAIN_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-main-context";
const SOURCE_NONCE_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-nonce-context";
const SOURCE_RECORD_STORE_SEAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-record-store-seal";
const SOURCE_ORDERED_RECORD_TOPOLOGY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-ordered-record-topology";
const SOURCE_PROVIDER_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-provider-receipt";
const SOURCE_SNAPSHOT_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-snapshot-receipt";
const SOURCE_PUBLICATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source-publication-receipt";
const CONFIDENTIAL_BACKEND_WIRED_V1: bool = true;
const PUBLIC_ARTIFACT_MANIFEST_BOUND_V1: bool = false;
const SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1: bool = false;
const SOURCE_ALGEBRA_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_MASKING_COMPLETE_V1: bool = false;
const Q_PCS_HANDOFF_COMPLETE_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;
const _: () = {
    assert!(SOURCE_VERSION_V1 == RNS_LINK_VERSION_V1);
    assert!(SOURCE_RECORD_COUNT_V1 == 43);
    assert!(SOURCE_EQUATIONS_PER_RECORD_V1 == 2);
    assert!(SOURCE_RELATION_COORDINATE_COUNT_V1 == 3_268);
    assert!(CANONICAL_COEFFICIENTS_PER_BLOCK_V1 == 256);
    assert!(SIGNED_COEFFICIENTS_PER_BLOCK_V1 == 1_024);
    assert!(CANONICAL_BLOCK_ENCODING_TAG_V1.len() <= u16::MAX as usize);
    assert!(SIGNED_BLOCK_ENCODING_TAG_V1.len() <= u16::MAX as usize);
    assert!(NONCE_ENCODING_TAG_V1.len() <= u16::MAX as usize);
    assert!(CANONICAL_BLOCKS_PER_RECORD_V1 == 512);
    assert!(SIGNED_BLOCKS_PER_POLYNOMIAL_V1 == 128);
    assert!(SECRET_MAIN_BLOCKS_PER_RECORD_V1 == 896);
    assert!(SECRET_MAIN_SLOT_COUNT_V1 == 38_528);
    assert!(SECRET_NONCE_SLOT_COUNT_V1 == 43);
    assert!(SECRET_MAIN_FILE_BYTES_V1 == 316_237_824);
    assert!(SECRET_NONCE_FILE_BYTES_V1 == 2_064);
    assert!(SECRET_TOTAL_FILE_BYTES_V1 == 316_239_888);
    assert!(CONFIDENTIAL_BACKEND_WIRED_V1);
    assert!(!PUBLIC_ARTIFACT_MANIFEST_BOUND_V1);
    assert!(!SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1);
    assert!(!SOURCE_ALGEBRA_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_MASKING_COMPLETE_V1);
    assert!(!Q_PCS_HANDOFF_COMPLETE_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_COMPLETE_V1);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalSourceRecordPositionV1 {
    ordinal: u16,
    family: ZkAmsPhase23RnsLinkFamilyV1,
    chunk_index: u16,
    family_chunk_count: u16,
    used_slots: u32,
}
const fn canonical_source_record_position_v1(
    ordinal: u16,
) -> Option<CanonicalSourceRecordPositionV1> {
    let (family, chunk_index, family_chunk_count, used_slots) = match ordinal {
        0 => (ZkAmsPhase23RnsLinkFamilyV1::X, 0, 1, 89),
        1..=16 => (
            ZkAmsPhase23RnsLinkFamilyV1::U,
            ordinal - 1,
            16,
            FULL_PACKED_USED_SLOTS_V1,
        ),
        17..=32 => (
            ZkAmsPhase23RnsLinkFamilyV1::E,
            ordinal - 17,
            16,
            FULL_PACKED_USED_SLOTS_V1,
        ),
        33 => (ZkAmsPhase23RnsLinkFamilyV1::RE, 0, 1, 1_024),
        34..=41 => (
            ZkAmsPhase23RnsLinkFamilyV1::W,
            ordinal - 34,
            8,
            FULL_PACKED_USED_SLOTS_V1,
        ),
        42 => (ZkAmsPhase23RnsLinkFamilyV1::RW, 0, 1, 512),
        _ => return None,
    };
    Some(CanonicalSourceRecordPositionV1 {
        ordinal,
        family,
        chunk_index,
        family_chunk_count,
        used_slots,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SecretRecordComponentV1 {
    CanonicalPlaintext = 1,
    Ephemeral = 2,
    ErrorZero = 3,
    ErrorOne = 4,
}
impl SecretRecordComponentV1 {
    const fn first_block(self) -> u16 {
        match self {
            Self::CanonicalPlaintext => 0,
            Self::Ephemeral => CANONICAL_BLOCKS_PER_RECORD_V1,
            Self::ErrorZero => CANONICAL_BLOCKS_PER_RECORD_V1 + SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
            Self::ErrorOne => CANONICAL_BLOCKS_PER_RECORD_V1 + 2 * SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        }
    }
    const fn block_count(self) -> u16 {
        match self {
            Self::CanonicalPlaintext => CANONICAL_BLOCKS_PER_RECORD_V1,
            Self::Ephemeral | Self::ErrorZero | Self::ErrorOne => SIGNED_BLOCKS_PER_POLYNOMIAL_V1,
        }
    }
    const fn encoding_v1(self) -> (&'static [u8], u64, u16) {
        match self {
            Self::CanonicalPlaintext => (
                CANONICAL_BLOCK_ENCODING_TAG_V1,
                CANONICAL_COEFFICIENT_BYTES_V1,
                CANONICAL_COEFFICIENTS_PER_BLOCK_V1,
            ),
            Self::Ephemeral | Self::ErrorZero | Self::ErrorOne => (
                SIGNED_BLOCK_ENCODING_TAG_V1,
                SIGNED_COEFFICIENT_BYTES_V1,
                SIGNED_COEFFICIENTS_PER_BLOCK_V1,
            ),
        }
    }
}
const SECRET_RECORD_COMPONENT_ORDER_V1: [SecretRecordComponentV1; 4] = [
    SecretRecordComponentV1::CanonicalPlaintext,
    SecretRecordComponentV1::Ephemeral,
    SecretRecordComponentV1::ErrorZero,
    SecretRecordComponentV1::ErrorOne,
];
const fn secret_main_slot_v1(
    record_ordinal: u16,
    component: SecretRecordComponentV1,
    component_block: u16,
) -> Option<u64> {
    if canonical_source_record_position_v1(record_ordinal).is_none()
        || component_block >= component.block_count()
    {
        return None;
    }
    Some(
        record_ordinal as u64 * SECRET_MAIN_BLOCKS_PER_RECORD_V1 as u64
            + component.first_block() as u64
            + component_block as u64,
    )
}
const fn secret_nonce_slot_v1(record_ordinal: u16) -> Option<u64> {
    if canonical_source_record_position_v1(record_ordinal).is_none() {
        None
    } else {
        Some(record_ordinal as u64)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SourceEquationV1 {
    Constant = 0,
    Linear = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SourcePublicKeyComponentV1 {
    CollectivePublicB = 1,
    CollectivePublicA = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SourceCiphertextComponentV1 {
    ConstantC0 = 1,
    LinearC1 = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceEquationPositionV1 {
    equation: SourceEquationV1,
    public_key_component: SourcePublicKeyComponentV1,
    ciphertext_component: SourceCiphertextComponentV1,
}
const SOURCE_EQUATION_ORDER_V1: [SourceEquationPositionV1; 2] = [
    SourceEquationPositionV1 {
        equation: SourceEquationV1::Constant,
        public_key_component: SourcePublicKeyComponentV1::CollectivePublicB,
        ciphertext_component: SourceCiphertextComponentV1::ConstantC0,
    },
    SourceEquationPositionV1 {
        equation: SourceEquationV1::Linear,
        public_key_component: SourcePublicKeyComponentV1::CollectivePublicA,
        ciphertext_component: SourceCiphertextComponentV1::LinearC1,
    },
];
const fn source_relation_coordinate_v1(
    record_ordinal: u16,
    equation: SourceEquationV1,
    limb: u16,
) -> Option<u32> {
    if canonical_source_record_position_v1(record_ordinal).is_none()
        || limb >= ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 as u16
    {
        return None;
    }
    Some(
        (record_ordinal as u32 * SOURCE_EQUATIONS_PER_RECORD_V1 as u32 + equation as u32)
            * ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 as u32
            + limb as u32,
    )
}
fn validate_source_release_geometry_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if geometry.rns_limb_count as usize != ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1
        || geometry.commitment_count != SOURCE_RECORD_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for (index, expected_family) in RNS_LINK_FAMILY_ORDER_V1.iter().copied().enumerate() {
        if geometry.families[index].family != expected_family {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    for ordinal in 0..SOURCE_RECORD_COUNT_V1 {
        let position = canonical_source_record_position_v1(ordinal)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let family = geometry.family(position.family)?;
        let expected_used_slots = if position.chunk_index + 1 == position.family_chunk_count {
            family.final_chunk_used_slots
        } else {
            FULL_PACKED_USED_SLOTS_V1
        };
        if family.family != position.family
            || family.chunk_count != position.family_chunk_count
            || position.chunk_index >= family.chunk_count
            || position.used_slots != expected_used_slots
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    Ok(())
}
fn source_mapping_digest_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_source_release_geometry_v1(geometry)?;
    source_mapping_digest_from_geometry_digest_v1(geometry.digest)
}
fn source_mapping_digest_from_geometry_digest_v1(
    geometry_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if geometry_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(SOURCE_MAPPING_DOMAIN_V1);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&geometry_digest);
    hash.update(&SOURCE_RECORD_COUNT_V1.to_be_bytes());
    hash.update(&SOURCE_EQUATIONS_PER_RECORD_V1.to_be_bytes());
    hash.update(&SOURCE_RELATION_COORDINATE_COUNT_V1.to_be_bytes());
    hash.update(&RING_DEGREE_V1.to_be_bytes());
    hash.update(&SECRET_MAIN_SLOT_COUNT_V1.to_be_bytes());
    hash.update(&SECRET_MAIN_PLAINTEXT_BYTES_V1.to_be_bytes());
    hash.update(&SECRET_NONCE_SLOT_COUNT_V1.to_be_bytes());
    hash.update(&SECRET_NONCE_PLAINTEXT_BYTES_V1.to_be_bytes());
    hash_source_encoding_v1(
        &mut hash,
        NONCE_ENCODING_TAG_V1,
        SECRET_NONCE_PLAINTEXT_BYTES_V1,
        1,
    );
    for ordinal in 0..SOURCE_RECORD_COUNT_V1 {
        let position = canonical_source_record_position_v1(ordinal)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        hash.update(&position.ordinal.to_be_bytes());
        hash.update(&[position.family as u8]);
        hash.update(&position.chunk_index.to_be_bytes());
        hash.update(&position.family_chunk_count.to_be_bytes());
        hash.update(&position.used_slots.to_be_bytes());
        for component in SECRET_RECORD_COMPONENT_ORDER_V1 {
            let (encoding_tag, element_width_bytes, elements_per_block) = component.encoding_v1();
            hash.update(&[component as u8]);
            hash.update(&component.first_block().to_be_bytes());
            hash.update(&component.block_count().to_be_bytes());
            hash_source_encoding_v1(
                &mut hash,
                encoding_tag,
                element_width_bytes,
                elements_per_block,
            );
        }
    }
    for position in SOURCE_EQUATION_ORDER_V1 {
        hash.update(&[position.equation as u8]);
        hash.update(&[position.public_key_component as u8]);
        hash.update(&[position.ciphertext_component as u8]);
    }
    hash.update(SOURCE_ABSOLUTE_MAIN_MAPPING_DOMAIN_V1);
    hash.update(&SECRET_MAIN_SLOT_COUNT_V1.to_be_bytes());
    for record in 0..SOURCE_RECORD_COUNT_V1 {
        for component in SECRET_RECORD_COMPONENT_ORDER_V1 {
            for component_block in 0..component.block_count() {
                let absolute_slot = secret_main_slot_v1(record, component, component_block)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
                hash.update(&record.to_be_bytes());
                hash.update(&[component as u8]);
                hash.update(&component_block.to_be_bytes());
                hash.update(&absolute_slot.to_be_bytes());
            }
        }
    }
    hash.update(SOURCE_ABSOLUTE_NONCE_MAPPING_DOMAIN_V1);
    hash.update(&SECRET_NONCE_SLOT_COUNT_V1.to_be_bytes());
    for record in 0..SOURCE_RECORD_COUNT_V1 {
        let absolute_slot =
            secret_nonce_slot_v1(record).ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        hash.update(&record.to_be_bytes());
        hash.update(&absolute_slot.to_be_bytes());
    }
    hash.update(SOURCE_ABSOLUTE_RELATION_MAPPING_DOMAIN_V1);
    hash.update(&SOURCE_RELATION_COORDINATE_COUNT_V1.to_be_bytes());
    for record in 0..SOURCE_RECORD_COUNT_V1 {
        for position in SOURCE_EQUATION_ORDER_V1 {
            for limb in 0..ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 as u16 {
                let absolute_coordinate =
                    source_relation_coordinate_v1(record, position.equation, limb)
                        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
                hash.update(&record.to_be_bytes());
                hash.update(&[position.equation as u8]);
                hash.update(&limb.to_be_bytes());
                hash.update(&absolute_coordinate.to_be_bytes());
            }
        }
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}
fn hash_source_encoding_v1(
    hash: &mut Keccak256,
    tag: &[u8],
    element_width_bytes: u64,
    element_count: u16,
) {
    hash.update(&(tag.len() as u16).to_be_bytes());
    hash.update(tag);
    hash.update(&element_width_bytes.to_be_bytes());
    hash.update(&element_count.to_be_bytes());
}
fn source_store_context_digest_v1(
    domain: &[u8],
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if context_digest == [0; 32] || mapping_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&context_digest);
    hash.update(&mapping_digest);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}
/// Move-only, zeroizing plaintext owner accepted by the source writer.
///
/// The only constructors allocate one exact main or nonce slot.  The mutable
/// borrow exists solely so the already-private Phase-23 producer can fill the
/// owned allocation; no publication or snapshot API returns raw borrowed
/// bytes.
#[must_use = "dropping this chunk zeroizes it without storing a source block"]
pub(in super::super) struct ZkAmsPhase23RnsLinkSecretChunkV1(ConfidentialSpoolChunkV1);
impl ZkAmsPhase23RnsLinkSecretChunkV1 {
    pub(in super::super) fn new_main_block_zeroed_v1() -> Result<Self, ZkAmsMkheErrorV1> {
        ConfidentialSpoolChunkV1::new_zeroed_v1(SECRET_MAIN_PLAINTEXT_BYTES_V1)
            .map(Self)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
    }
    pub(in super::super) fn new_nonce_zeroed_v1() -> Result<Self, ZkAmsMkheErrorV1> {
        ConfidentialSpoolChunkV1::new_zeroed_v1(SECRET_NONCE_PLAINTEXT_BYTES_V1)
            .map(Self)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)
    }
    pub(in super::super) fn as_mut_bytes_v1(&mut self) -> &mut [u8] {
        self.0.as_mut_slice_v1()
    }
}
fn stored_record_digest_v1(live: &LiveExternalSourceAssemblyV1, record_ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_RECORD_STORE_SEAL_DOMAIN_V1);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&live.backend.writer_identity_v1());
    hash.update(&live.main_context_digest);
    hash.update(&live.nonce_context_digest);
    hash.update(&record_ordinal.to_be_bytes());
    hash.update(&live.next_main_slot.to_be_bytes());
    hash.update(&live.next_nonce_slot.to_be_bytes());
    hash.finalize()
}
/// Immutable, non-authorizing identity of the live confidential provider.
pub(in super::super) struct ZkAmsPhase23RnsLinkSourceProviderReceiptV1 {
    provider_identity: [u8; 32],
    writer_identity: [u8; 32],
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    receipt_digest: [u8; 32],
}
/// Immutable, non-authorizing identity of both sealed confidential snapshots.
pub(in super::super) struct ZkAmsPhase23RnsLinkSourceSnapshotReceiptV1 {
    provider_receipt_digest: [u8; 32],
    snapshot_identity: [u8; 32],
    main_snapshot_digest: [u8; 32],
    nonce_snapshot_digest: [u8; 32],
    main_file_bytes: u64,
    nonce_file_bytes: u64,
    receipt_digest: [u8; 32],
}
/// Immutable structural publication metadata.
///
/// Only concrete confidential-backend wiring may be true.  Every stronger
/// completion bit stays false.  No release consumer accepts this type, and it
/// has no decoder or detached-digest constructor.
pub(in super::super) struct ZkAmsPhase23RnsLinkSourcePublicationReceiptV1 {
    provider: ZkAmsPhase23RnsLinkSourceProviderReceiptV1,
    snapshot: ZkAmsPhase23RnsLinkSourceSnapshotReceiptV1,
    publication_identity: [u8; 32],
    ordered_record_topology_root: [u8; 32],
    record_count: u16,
    relation_coordinate_count: u32,
    confidential_backend_wired: bool,
    public_artifact_manifest_bound: bool,
    source_relation_polynomials_constructed: bool,
    source_algebra_verified: bool,
    zero_knowledge_masking_complete: bool,
    q_pcs_handoff_complete: bool,
    operational_receipt_accepted: bool,
    release_complete: bool,
    receipt_digest: [u8; 32],
}
impl ZkAmsPhase23RnsLinkSourcePublicationReceiptV1 {
    pub(in super::super) const fn receipt_digest_v1(&self) -> [u8; 32] {
        self.receipt_digest
    }
}
fn provider_receipt_digest_v1(receipt: &ZkAmsPhase23RnsLinkSourceProviderReceiptV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_PROVIDER_RECEIPT_DOMAIN_V1);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&receipt.provider_identity);
    hash.update(&receipt.writer_identity);
    hash.update(&receipt.context_digest);
    hash.update(&receipt.mapping_digest);
    hash.finalize()
}
fn snapshot_receipt_digest_v1(receipt: &ZkAmsPhase23RnsLinkSourceSnapshotReceiptV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_SNAPSHOT_RECEIPT_DOMAIN_V1);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&receipt.provider_receipt_digest);
    hash.update(&receipt.snapshot_identity);
    hash.update(&receipt.main_snapshot_digest);
    hash.update(&receipt.nonce_snapshot_digest);
    hash.update(&receipt.main_file_bytes.to_be_bytes());
    hash.update(&receipt.nonce_file_bytes.to_be_bytes());
    hash.finalize()
}
fn publication_receipt_digest_v1(
    receipt: &ZkAmsPhase23RnsLinkSourcePublicationReceiptV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_PUBLICATION_RECEIPT_DOMAIN_V1);
    hash.update(&[SOURCE_VERSION_V1]);
    hash.update(&receipt.provider.receipt_digest);
    hash.update(&receipt.snapshot.receipt_digest);
    hash.update(&receipt.publication_identity);
    hash.update(&receipt.ordered_record_topology_root);
    hash.update(&receipt.record_count.to_be_bytes());
    hash.update(&receipt.relation_coordinate_count.to_be_bytes());
    hash.update(&[
        receipt.confidential_backend_wired as u8,
        receipt.public_artifact_manifest_bound as u8,
        receipt.source_relation_polynomials_constructed as u8,
        receipt.source_algebra_verified as u8,
        receipt.zero_knowledge_masking_complete as u8,
        receipt.q_pcs_handoff_complete as u8,
        receipt.operational_receipt_accepted as u8,
        receipt.release_complete as u8,
    ]);
    hash.finalize()
}
/// Move-only, poison-on-failure assembly of exactly 43 stored records.
#[must_use = "dropping this assembly publishes no confidential source"]
pub(in super::super) struct ZkAmsPhase23RnsLinkExternalSourceAssemblyV1 {
    live: Option<LiveExternalSourceAssemblyV1>,
}
struct LiveExternalSourceAssemblyV1 {
    backend: RnsLinkSecretSpoolWriterV1,
    context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    main_context_digest: [u8; 32],
    nonce_context_digest: [u8; 32],
    next_record: u16,
    next_main_slot: u64,
    next_nonce_slot: u64,
    ordered_record_topology_hash: Keccak256,
}
impl ZkAmsPhase23RnsLinkExternalSourceAssemblyV1 {
    /// Create both exact unlinked confidential spools from a validated context.
    ///
    /// The directory is borrowed only during creation and is never retained in
    /// source state, receipts, identities, or errors.
    pub(in super::super) fn begin_v1(
        context: ZkAmsPhase23RnsLinkContextV1,
        directory: impl AsRef<Path>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let geometry = derive_zk_ams_phase23_rns_link_release_geometry_v1()?;
        validate_source_release_geometry_v1(&geometry)?;
        let context_digest = context.digest();
        let mapping_digest = source_mapping_digest_v1(&geometry)?;
        let main_context_digest = source_store_context_digest_v1(
            SOURCE_MAIN_CONTEXT_DOMAIN_V1,
            context_digest,
            mapping_digest,
        )?;
        let nonce_context_digest = source_store_context_digest_v1(
            SOURCE_NONCE_CONTEXT_DOMAIN_V1,
            context_digest,
            mapping_digest,
        )?;
        let backend = RnsLinkSecretSpoolWriterV1::create_v1(
            directory.as_ref(),
            context_digest,
            geometry.digest,
            mapping_digest,
            main_context_digest,
            nonce_context_digest,
        )?;
        let mut ordered_record_topology_hash = Keccak256::new();
        ordered_record_topology_hash.update(SOURCE_ORDERED_RECORD_TOPOLOGY_DOMAIN_V1);
        ordered_record_topology_hash.update(&[SOURCE_VERSION_V1]);
        ordered_record_topology_hash.update(&context_digest);
        ordered_record_topology_hash.update(&geometry.digest);
        ordered_record_topology_hash.update(&mapping_digest);
        ordered_record_topology_hash.update(&backend.writer_identity_v1());
        Ok(Self {
            live: Some(LiveExternalSourceAssemblyV1 {
                backend,
                context_digest,
                mapping_digest,
                main_context_digest,
                nonce_context_digest,
                next_record: 0,
                next_main_slot: 0,
                next_nonce_slot: 0,
                ordered_record_topology_hash,
            }),
        })
    }
    fn write_next_main_block_v1(
        &mut self,
        record_ordinal: u16,
        component: SecretRecordComponentV1,
        component_block: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if record_ordinal != live.next_record
            || live.next_nonce_slot != u64::from(record_ordinal)
            || secret_main_slot_v1(record_ordinal, component, component_block)
                != Some(live.next_main_slot)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.backend.write_main_v1(live.next_main_slot, chunk.0)?;
        live.next_main_slot = live
            .next_main_slot
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.live = Some(live);
        Ok(())
    }
    pub(in super::super) fn write_next_canonical_plaintext_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.write_next_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::CanonicalPlaintext,
            component_block,
            chunk,
        )
    }
    pub(in super::super) fn write_next_ephemeral_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.write_next_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::Ephemeral,
            component_block,
            chunk,
        )
    }
    pub(in super::super) fn write_next_error_zero_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.write_next_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::ErrorZero,
            component_block,
            chunk,
        )
    }
    pub(in super::super) fn write_next_error_one_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.write_next_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::ErrorOne,
            component_block,
            chunk,
        )
    }
    /// Persist the nonce only after all 896 main blocks for this record.
    ///
    /// Every write method removes live state before coordinate validation or
    /// I/O.  An error or unwind therefore drops both writers and forbids retry.
    pub(in super::super) fn write_next_nonce_v1(
        &mut self,
        record_ordinal: u16,
        chunk: ZkAmsPhase23RnsLinkSecretChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let record_after = record_ordinal
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_main_slots = u64::from(record_after)
            .checked_mul(u64::from(SECRET_MAIN_BLOCKS_PER_RECORD_V1))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if record_ordinal != live.next_record
            || live.next_main_slot != expected_main_slots
            || secret_nonce_slot_v1(record_ordinal) != Some(live.next_nonce_slot)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.backend.write_nonce_v1(live.next_nonce_slot, chunk.0)?;
        live.next_nonce_slot = live
            .next_nonce_slot
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let position = canonical_source_record_position_v1(record_ordinal)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let record_digest = stored_record_digest_v1(&live, record_ordinal);
        if record_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.ordered_record_topology_hash.update(&record_digest);
        live.ordered_record_topology_hash
            .update(&position.ordinal.to_be_bytes());
        live.ordered_record_topology_hash
            .update(&[position.family as u8]);
        live.ordered_record_topology_hash
            .update(&position.chunk_index.to_be_bytes());
        live.next_record = record_after;
        self.live = Some(live);
        Ok(())
    }
    /// Seal both exact snapshots only after all 43 records are complete.
    pub(in super::super) fn finish_v1(
        self,
    ) -> Result<ZkAmsPhase23RnsLinkExternalSourcePublicationV1, ZkAmsMkheErrorV1> {
        let live = self.live.ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_record != SOURCE_RECORD_COUNT_V1
            || live.next_main_slot != SECRET_MAIN_SLOT_COUNT_V1
            || live.next_nonce_slot != SECRET_NONCE_SLOT_COUNT_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let ordered_record_topology_root = live.ordered_record_topology_hash.finalize();
        if ordered_record_topology_root == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let backend = live.backend.seal_v1(ordered_record_topology_root)?;
        if backend.main_file_bytes_v1() != SECRET_MAIN_FILE_BYTES_V1
            || backend.nonce_file_bytes_v1() != SECRET_NONCE_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut provider = ZkAmsPhase23RnsLinkSourceProviderReceiptV1 {
            provider_identity: backend.provider_identity_v1(),
            writer_identity: backend.writer_identity_v1(),
            context_digest: live.context_digest,
            mapping_digest: live.mapping_digest,
            receipt_digest: [0; 32],
        };
        provider.receipt_digest = provider_receipt_digest_v1(&provider);
        let mut snapshot = ZkAmsPhase23RnsLinkSourceSnapshotReceiptV1 {
            provider_receipt_digest: provider.receipt_digest,
            snapshot_identity: backend.snapshot_identity_v1(),
            main_snapshot_digest: backend.main_snapshot_digest_v1(),
            nonce_snapshot_digest: backend.nonce_snapshot_digest_v1(),
            main_file_bytes: backend.main_file_bytes_v1(),
            nonce_file_bytes: backend.nonce_file_bytes_v1(),
            receipt_digest: [0; 32],
        };
        snapshot.receipt_digest = snapshot_receipt_digest_v1(&snapshot);
        let mut receipt = ZkAmsPhase23RnsLinkSourcePublicationReceiptV1 {
            provider,
            snapshot,
            publication_identity: backend.publication_identity_v1(),
            ordered_record_topology_root,
            record_count: live.next_record,
            relation_coordinate_count: SOURCE_RELATION_COORDINATE_COUNT_V1,
            confidential_backend_wired: CONFIDENTIAL_BACKEND_WIRED_V1,
            public_artifact_manifest_bound: PUBLIC_ARTIFACT_MANIFEST_BOUND_V1,
            source_relation_polynomials_constructed: SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1,
            source_algebra_verified: SOURCE_ALGEBRA_VERIFIED_V1,
            zero_knowledge_masking_complete: ZERO_KNOWLEDGE_MASKING_COMPLETE_V1,
            q_pcs_handoff_complete: Q_PCS_HANDOFF_COMPLETE_V1,
            operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V1,
            release_complete: RELEASE_COMPLETE_V1,
            receipt_digest: [0; 32],
        };
        receipt.receipt_digest = publication_receipt_digest_v1(&receipt);
        if receipt.provider.receipt_digest == [0; 32]
            || receipt.snapshot.receipt_digest == [0; 32]
            || receipt.receipt_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(ZkAmsPhase23RnsLinkExternalSourcePublicationV1 { backend, receipt })
    }
}
/// Move-only owner of both authenticated snapshots and immutable receipt.
#[must_use = "dropping this publication produces no RNS-Link relation proof"]
pub(in super::super) struct ZkAmsPhase23RnsLinkExternalSourcePublicationV1 {
    backend: RnsLinkSecretSpoolSnapshotsV1,
    receipt: ZkAmsPhase23RnsLinkSourcePublicationReceiptV1,
}
impl ZkAmsPhase23RnsLinkExternalSourcePublicationV1 {
    pub(in super::super) const fn receipt_v1(
        &self,
    ) -> &ZkAmsPhase23RnsLinkSourcePublicationReceiptV1 {
        &self.receipt
    }
    fn read_main_block_v1(
        &mut self,
        record_ordinal: u16,
        component: SecretRecordComponentV1,
        component_block: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        let slot = secret_main_slot_v1(record_ordinal, component, component_block)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.backend
            .read_main_v1(slot)
            .map(ZkAmsPhase23RnsLinkSecretChunkV1)
    }
    pub(in super::super) fn read_canonical_plaintext_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        self.read_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::CanonicalPlaintext,
            component_block,
        )
    }
    pub(in super::super) fn read_ephemeral_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        self.read_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::Ephemeral,
            component_block,
        )
    }
    pub(in super::super) fn read_error_zero_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        self.read_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::ErrorZero,
            component_block,
        )
    }
    pub(in super::super) fn read_error_one_block_v1(
        &mut self,
        record_ordinal: u16,
        component_block: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        self.read_main_block_v1(
            record_ordinal,
            SecretRecordComponentV1::ErrorOne,
            component_block,
        )
    }
    pub(in super::super) fn read_nonce_v1(
        &mut self,
        record_ordinal: u16,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        let slot =
            secret_nonce_slot_v1(record_ordinal).ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.backend
            .read_nonce_v1(slot)
            .map(ZkAmsPhase23RnsLinkSecretChunkV1)
    }
}
/// Static accounting facts, not runtime or release evidence.
struct ExternalSourceLinkPlanV1 {
    release_family_count: usize,
    release_record_count: usize,
    release_rns_limb_count: usize,
    native_equation_count: usize,
    native_relation_coordinate_count: usize,
    current_state_owner_bytes: u64,
    prior_full_mirror_bytes: u64,
    secret_main_slot_count: u64,
    secret_nonce_slot_count: u64,
    secret_main_file_bytes: u64,
    secret_nonce_file_bytes: u64,
    secret_total_file_bytes: u64,
    named_persistent_slot_cursor_bytes: u64,
    max_single_owned_chunk_bytes: u64,
    proposed_specialized_encryption_bytes: u64,
    masked_q_pcs_isolated_heap_bytes: u64,
    named_combined_heap_bytes: u64,
    confidential_backend_wired: bool,
    public_artifact_manifest_bound: bool,
    source_relation_polynomials_constructed: bool,
    source_algebra_verified: bool,
    zero_knowledge_masking_complete: bool,
    q_pcs_handoff_complete: bool,
    operational_receipt_accepted: bool,
    release_complete: bool,
}
const EXTERNAL_SOURCE_LINK_PLAN_V1: ExternalSourceLinkPlanV1 = ExternalSourceLinkPlanV1 {
    release_family_count: 6,
    release_record_count: 43,
    release_rns_limb_count: 38,
    native_equation_count: 86,
    native_relation_coordinate_count: 3_268,
    current_state_owner_bytes: 3_686_793_216,
    prior_full_mirror_bytes: 3_829_526_544,
    secret_main_slot_count: SECRET_MAIN_SLOT_COUNT_V1,
    secret_nonce_slot_count: SECRET_NONCE_SLOT_COUNT_V1,
    secret_main_file_bytes: SECRET_MAIN_FILE_BYTES_V1,
    secret_nonce_file_bytes: SECRET_NONCE_FILE_BYTES_V1,
    secret_total_file_bytes: SECRET_TOTAL_FILE_BYTES_V1,
    named_persistent_slot_cursor_bytes: 16,
    max_single_owned_chunk_bytes: SECRET_MAIN_PLAINTEXT_BYTES_V1,
    proposed_specialized_encryption_bytes: 9_445_392,
    masked_q_pcs_isolated_heap_bytes: 74_662_064,
    named_combined_heap_bytes: 84_107_456,
    confidential_backend_wired: CONFIDENTIAL_BACKEND_WIRED_V1,
    public_artifact_manifest_bound: PUBLIC_ARTIFACT_MANIFEST_BOUND_V1,
    source_relation_polynomials_constructed: SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V1,
    source_algebra_verified: SOURCE_ALGEBRA_VERIFIED_V1,
    zero_knowledge_masking_complete: ZERO_KNOWLEDGE_MASKING_COMPLETE_V1,
    q_pcs_handoff_complete: Q_PCS_HANDOFF_COMPLETE_V1,
    operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V1,
    release_complete: RELEASE_COMPLETE_V1,
};
#[cfg(test)]
#[path = "phase23_rns_link_external_source_tests.rs"]
mod tests;
