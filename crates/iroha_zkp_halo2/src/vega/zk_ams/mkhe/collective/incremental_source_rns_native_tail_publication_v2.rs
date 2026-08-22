//! Source-only CAS publication and typed reader handoff for the 176 genuine
//! 38-to-40-limb tail objects.
//!
//! This child consumes the arithmetic owner from
//! `incremental_source_rns_native_basis_extension_v2`, publishes only
//! `A[38..40]`, `B[38..40]`, and the 43 record-local `C0/C1[38..40]` tails,
//! and retains every real [`ZkAmsMkheDirectObjectPublicationReceiptV1`]. It
//! never republishes the frozen 3,344-object V1 prefix. The exact whole V1
//! authority/manifest shape is represented below, but has no constructor or
//! production adapter: Phase-23 and the synchronous V1 callback remain absent.
//!
//! The composite reader provider derives fresh domain-separated identities
//! from the current key/ciphertext provider sessions and snapshots. Historical
//! receipt snapshots are retained as provenance but are deliberately not
//! required to equal the final composite snapshot. Every live call instead
//! rejects current-axis drift, unknown pointers, route confusion, length
//! mutation, and short reads.
//!
//! This module closes no integration, resource-evidence, readiness, qPCS,
//! admission, or release gate.

#![allow(
    dead_code,
    reason = "private source-only tail publication contract awaits the live V1/Phase-23 owner"
)]

use core::convert::Infallible;
use std::collections::{BTreeMap, BTreeSet};

use super::super::super::{
    ZkAmsMkheErrorV1,
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1,
        ZkAmsMkheDirectObjectCasPublicationV1, ZkAmsMkheDirectObjectKindV1,
        ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectPublicationReceiptV1,
        ZkAmsMkheDirectObjectPublicationTransactionV1, ZkAmsMkheDirectObjectReadAtProviderV1,
        ZkAmsMkheDirectObjectReadReceiptV1,
    },
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1,
    },
    rns_native_public_polynomial_reader::{
        RnsNativePublicPolynomialDescriptorV1, RnsNativePublicPolynomialEvaluationV1,
        RnsNativePublicPolynomialManifestV1, RnsNativePublicPolynomialReadReceiptV1,
        RnsNativePublicPolynomialReaderErrorV1, RnsNativePublicPolynomialReaderV1,
        RnsNativePublicPolynomialRoleV1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
};
use super::{
    ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    incremental_source_rns_native_basis_extension_v2::{
        RnsNativeBasisExtensionErrorV2, RnsNativeCiphertextTailAggregateChecksumV2,
        RnsNativeCiphertextTailCompletionV2, RnsNativeCiphertextTailLifecycleV2,
        RnsNativeCiphertextTailWorkspaceOwnerV2, RnsNativeCollectiveKeyTailOwnerV2,
        RnsNativeEncodedTailObjectV2, RnsNativePublishedCollectiveKeyTailOwnerV2,
        RnsNativeTailCoefficientVisitorV2, RnsNativeTailObjectEncoderV2, RnsNativeTailObjectRoleV2,
        RnsNativeTailSourcePositionV2,
    },
};
use crate::vega::sponge::Keccak256;

const VERSION_V2: u8 = 2;
const LEGACY_LIMBS_V2: usize = 38;
const TARGET_LIMBS_V2: usize = 40;
const RECORDS_V2: usize = 43;
const KEY_TAIL_OBJECTS_V2: usize = 4;
const TAIL_OBJECTS_PER_RECORD_V2: usize = 4;
const CIPHERTEXT_TAIL_OBJECTS_V2: usize = RECORDS_V2 * TAIL_OBJECTS_PER_RECORD_V2;
const TAIL_OBJECTS_V2: usize = KEY_TAIL_OBJECTS_V2 + CIPHERTEXT_TAIL_OBJECTS_V2;
const PREFIX_KEY_OBJECTS_V2: usize = 2 * LEGACY_LIMBS_V2;
const PREFIX_OBJECTS_PER_RECORD_V2: usize = 2 * LEGACY_LIMBS_V2;
const PREFIX_CIPHERTEXT_OBJECTS_V2: usize = RECORDS_V2 * PREFIX_OBJECTS_PER_RECORD_V2;
const PREFIX_OBJECTS_V2: usize = PREFIX_KEY_OBJECTS_V2 + PREFIX_CIPHERTEXT_OBJECTS_V2;
const FULL_OBJECTS_V2: usize = PREFIX_OBJECTS_V2 + TAIL_OBJECTS_V2;
const COUNT_PREFIX_BYTES_V2: usize = core::mem::size_of::<u32>();
const COEFFICIENT_BYTES_V2: usize = core::mem::size_of::<u64>();
const COEFFICIENTS_PER_CHUNK_V2: usize =
    ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / COEFFICIENT_BYTES_V2;
const CHUNKS_PER_OBJECT_V2: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / COEFFICIENTS_PER_CHUNK_V2;
const OBJECT_BYTES_V2: usize =
    COUNT_PREFIX_BYTES_V2 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * COEFFICIENT_BYTES_V2;
const WRITES_PER_OBJECT_V2: usize = 1 + CHUNKS_PER_OBJECT_V2;
const PUBLICATION_RECEIPT_OWNER_BYTES_V2: usize =
    core::mem::size_of::<ZkAmsMkheDirectObjectPublicationReceiptV1>();
const READ_RECEIPT_OWNER_BYTES_V2: usize =
    core::mem::size_of::<ZkAmsMkheDirectObjectReadReceiptV1>();
const RETAINED_V1_READ_RECEIPTS_V2: usize = 4 * RECORDS_V2 * LEGACY_LIMBS_V2;
const QPCS_REPETITIONS_V2: usize = 5;
const QPCS_EVALUATIONS_V2: usize = TARGET_LIMBS_V2 * QPCS_REPETITIONS_V2;

const TAIL_LIFECYCLE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native.tail-publication.lifecycle";
const COMPOSITE_PROVIDER_IDENTITY_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native.tail-publication.composite-provider";
const COMPOSITE_SNAPSHOT_IDENTITY_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native.tail-publication.composite-snapshot";

/// Exact contracts implemented by this private source-only child.
pub(super) const RNS_NATIVE_TAIL_CAS_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_TAIL_LIFECYCLE_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_WHOLE_V1_OWNER_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_COMPOSITE_PROVIDER_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_EXISTING_READER_BRIDGE_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_SINGLE_QPCS_BATCH_CONTRACT_IMPLEMENTED_V2: bool = true;

/// Live and evidence gates remain fail-closed.
pub(super) const RNS_NATIVE_TAIL_V1_CALLBACK_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_PHASE23_OWNER_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_KEY_TAIL_CAS_OWNER_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_PRODUCTION_OWNER_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_PRODUCTION_ADAPTER_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_COMPOSITE_PROVIDER_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_EXISTING_READER_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_SINGLE_QPCS_BATCH_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_READER_PROVIDER_RETURN_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_DEVICE_EVIDENCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_READINESS_V2: bool = false;
pub(super) const RNS_NATIVE_TAIL_RELEASE_AUTHORIZED_V2: bool = false;

/// Exact static ledger for tail publication plus the existing complete reader.
pub(super) struct RnsNativeTailPublicationResourceLedgerV2 {
    pub(super) tail_objects: u16,
    pub(super) tail_coefficients: u64,
    pub(super) tail_canonical_bytes: u64,
    pub(super) tail_coefficient_chunks: u32,
    pub(super) tail_publication_writes: u32,
    pub(super) tail_transport_operations: u32,
    pub(super) tail_authenticated_transfer_bytes: u64,
    pub(super) tail_work_units: u64,
    pub(super) tail_pointer_frame_bytes: u32,
    pub(super) tail_publication_receipt_bytes: u32,
    pub(super) all_publication_receipt_bytes: u32,
    pub(super) retained_v1_read_receipt_bytes: u32,
    pub(super) tail_plus_reader_io_bytes: u64,
    pub(super) tail_plus_reader_work_units: u64,
}

pub(super) const RNS_NATIVE_TAIL_PUBLICATION_RESOURCE_LEDGER_V2:
    RnsNativeTailPublicationResourceLedgerV2 = RnsNativeTailPublicationResourceLedgerV2 {
    tail_objects: 176,
    tail_coefficients: 23_068_672,
    tail_canonical_bytes: 184_550_080,
    tail_coefficient_chunks: 22_528,
    tail_publication_writes: 22_704,
    tail_transport_operations: 68_112,
    tail_authenticated_transfer_bytes: 553_650_240,
    tail_work_units: 576_718_912,
    tail_pointer_frame_bytes: 13_728,
    tail_publication_receipt_bytes: (TAIL_OBJECTS_V2 * PUBLICATION_RECEIPT_OWNER_BYTES_V2) as u32,
    all_publication_receipt_bytes: (FULL_OBJECTS_V2 * PUBLICATION_RECEIPT_OWNER_BYTES_V2) as u32,
    retained_v1_read_receipt_bytes: (RETAINED_V1_READ_RECEIPTS_V2 * READ_RECEIPT_OWNER_BYTES_V2)
        as u32,
    tail_plus_reader_io_bytes: 4_244_651_840,
    tail_plus_reader_work_units: 9_349_571_552,
};

const _: () = {
    assert!(LEGACY_LIMBS_V2 == 38);
    assert!(TARGET_LIMBS_V2 == ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1);
    assert!(RECORDS_V2 == 43);
    assert!(KEY_TAIL_OBJECTS_V2 == 4);
    assert!(TAIL_OBJECTS_PER_RECORD_V2 == 4);
    assert!(CIPHERTEXT_TAIL_OBJECTS_V2 == 172);
    assert!(TAIL_OBJECTS_V2 == 176);
    assert!(PREFIX_KEY_OBJECTS_V2 == 76);
    assert!(PREFIX_CIPHERTEXT_OBJECTS_V2 == 3_268);
    assert!(PREFIX_OBJECTS_V2 == 3_344);
    assert!(FULL_OBJECTS_V2 == 3_520);
    assert!(COEFFICIENTS_PER_CHUNK_V2 == 1_024);
    assert!(CHUNKS_PER_OBJECT_V2 == 128);
    assert!(OBJECT_BYTES_V2 == 1_048_580);
    assert!(WRITES_PER_OBJECT_V2 == 129);
    assert!(PUBLICATION_RECEIPT_OWNER_BYTES_V2 == 704);
    assert!(READ_RECEIPT_OWNER_BYTES_V2 == 248);
    assert!(RETAINED_V1_READ_RECEIPTS_V2 == 6_536);
    assert!(QPCS_EVALUATIONS_V2 == 200);
    assert!(TAIL_OBJECTS_V2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 23_068_672);
    assert!(TAIL_OBJECTS_V2 * OBJECT_BYTES_V2 == 184_550_080);
    assert!(TAIL_OBJECTS_V2 * CHUNKS_PER_OBJECT_V2 == 22_528);
    assert!(TAIL_OBJECTS_V2 * WRITES_PER_OBJECT_V2 == 22_704);
    assert!(3 * TAIL_OBJECTS_V2 * WRITES_PER_OBJECT_V2 == 68_112);
    assert!(3 * TAIL_OBJECTS_V2 * OBJECT_BYTES_V2 == 553_650_240);
    assert!(553_650_240 + 23_068_672 == 576_718_912);
    assert!(TAIL_OBJECTS_V2 * ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1 == 13_728);
    assert!(TAIL_OBJECTS_V2 * PUBLICATION_RECEIPT_OWNER_BYTES_V2 == 123_904);
    assert!(FULL_OBJECTS_V2 * PUBLICATION_RECEIPT_OWNER_BYTES_V2 == 2_478_080);
    assert!(RETAINED_V1_READ_RECEIPTS_V2 * READ_RECEIPT_OWNER_BYTES_V2 == 1_620_928);
    assert!(553_650_240 + 3_691_001_600 == 4_244_651_840);
    assert!(576_718_912 + 8_772_852_640 == 9_349_571_552);
    assert!(
        RNS_NATIVE_TAIL_PUBLICATION_RESOURCE_LEDGER_V2.tail_plus_reader_io_bytes
            < ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1
    );
    assert!(
        RNS_NATIVE_TAIL_PUBLICATION_RESOURCE_LEDGER_V2.tail_plus_reader_work_units
            < ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1
    );
    assert!(RNS_NATIVE_TAIL_CAS_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_TAIL_LIFECYCLE_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_WHOLE_V1_OWNER_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_COMPOSITE_PROVIDER_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_EXISTING_READER_BRIDGE_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_SINGLE_QPCS_BATCH_CONTRACT_IMPLEMENTED_V2);
    assert!(!RNS_NATIVE_TAIL_V1_CALLBACK_INTEGRATED_V2);
    assert!(!RNS_NATIVE_TAIL_PHASE23_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_KEY_TAIL_CAS_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_TAIL_PRODUCTION_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_TAIL_PRODUCTION_ADAPTER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_COMPOSITE_PROVIDER_INTEGRATED_V2);
    assert!(!RNS_NATIVE_EXISTING_READER_INTEGRATED_V2);
    assert!(!RNS_NATIVE_SINGLE_QPCS_BATCH_INTEGRATED_V2);
    assert!(!RNS_NATIVE_READER_PROVIDER_RETURN_INTEGRATED_V2);
    assert!(!RNS_NATIVE_TAIL_RESOURCE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_TAIL_DEVICE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_TAIL_READINESS_V2);
    assert!(!RNS_NATIVE_TAIL_RELEASE_AUTHORIZED_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeTailPublicationBlockerV2 {
    pub(super) code: &'static str,
    pub(super) required_delta: &'static str,
}

pub(super) const RNS_NATIVE_TAIL_PUBLICATION_BLOCKERS_V2: &[RnsNativeTailPublicationBlockerV2] = &[
    RnsNativeTailPublicationBlockerV2 {
        code: "LIVE_CPK_KEY_CAS_OWNER",
        required_delta: "consume a CPK-finalization K:CasPublication owner for A/B tails; the retained Phase23 K is read-only and cannot be promoted",
    },
    RnsNativeTailPublicationBlockerV2 {
        code: "LIVE_V1_SAME_OPENING_CALLBACK",
        required_delta: "allocate each two-limb workspace before V1 entropy and synchronously move the exact m,r,e0,e1,nonce opening through this private lifecycle",
    },
    RnsNativeTailPublicationBlockerV2 {
        code: "WHOLE_PHASE23_V1_OWNER",
        required_delta: "consume the one whole streaming key authority and 43 whole ordered ciphertext manifests without cloning their 3,344 prefix receipts",
    },
    RnsNativeTailPublicationBlockerV2 {
        code: "READER_PROVIDER_RETURN",
        required_delta: "review and add an existing-reader finish-with-provider transition if the next qPCS stage must retain the composite provider; current finish returns only its read receipt",
    },
    RnsNativeTailPublicationBlockerV2 {
        code: "RESOURCE_AND_DEVICE_EVIDENCE",
        required_delta: "qualify the pinned aggregate ledgers with measured RSS, device, interoperability, and full ignored-gate evidence",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeTailPublicationErrorV2 {
    Basis,
    Transport,
    InvalidPosition,
    InvalidOrder,
    InvalidReceipt,
    InvalidV1Owner,
    DuplicatePointer,
    ProviderAxes,
    UnknownPointer,
    WrongRoute,
    LengthMutation,
    ShortRead,
    Reader,
    ResourceCeiling,
    Incomplete,
    Poisoned,
}

fn reader_role_v2(position: RnsNativeTailSourcePositionV2) -> RnsNativePublicPolynomialRoleV1 {
    match position.role_v2() {
        RnsNativeTailObjectRoleV2::PublicA => RnsNativePublicPolynomialRoleV1::PublicA,
        RnsNativeTailObjectRoleV2::CollectivePublicB => RnsNativePublicPolynomialRoleV1::PublicB,
        RnsNativeTailObjectRoleV2::CiphertextC0 => RnsNativePublicPolynomialRoleV1::CiphertextC0,
        RnsNativeTailObjectRoleV2::CiphertextC1 => RnsNativePublicPolynomialRoleV1::CiphertextC1,
    }
}

fn tail_position_v2(
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<usize>,
    limb: usize,
) -> Result<RnsNativeTailSourcePositionV2, RnsNativeTailPublicationErrorV2> {
    let tail = limb
        .checked_sub(LEGACY_LIMBS_V2)
        .filter(|tail| *tail < TARGET_LIMBS_V2 - LEGACY_LIMBS_V2)
        .ok_or(RnsNativeTailPublicationErrorV2::InvalidPosition)?;
    let ordinal = match (role, record) {
        (RnsNativePublicPolynomialRoleV1::PublicA, None) => tail,
        (RnsNativePublicPolynomialRoleV1::PublicB, None) => 2 + tail,
        (RnsNativePublicPolynomialRoleV1::CiphertextC0, Some(record)) if record < RECORDS_V2 => {
            KEY_TAIL_OBJECTS_V2 + record * 2 + tail
        }
        (RnsNativePublicPolynomialRoleV1::CiphertextC1, Some(record)) if record < RECORDS_V2 => {
            KEY_TAIL_OBJECTS_V2 + RECORDS_V2 * 2 + record * 2 + tail
        }
        _ => return Err(RnsNativeTailPublicationErrorV2::InvalidPosition),
    };
    RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(ordinal)
        .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition)
}

pub(super) fn physical_tail_position_v2(
    physical_ordinal: usize,
) -> Result<RnsNativeTailSourcePositionV2, RnsNativeTailPublicationErrorV2> {
    if physical_ordinal < KEY_TAIL_OBJECTS_V2 {
        return RnsNativeTailSourcePositionV2::from_tail_ordinal_v2(physical_ordinal)
            .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition);
    }
    let relative = physical_ordinal
        .checked_sub(KEY_TAIL_OBJECTS_V2)
        .ok_or(RnsNativeTailPublicationErrorV2::InvalidPosition)?;
    let record = relative / TAIL_OBJECTS_PER_RECORD_V2;
    let within = relative % TAIL_OBJECTS_PER_RECORD_V2;
    if record >= RECORDS_V2 {
        return Err(RnsNativeTailPublicationErrorV2::InvalidPosition);
    }
    let role = if within < 2 {
        RnsNativePublicPolynomialRoleV1::CiphertextC0
    } else {
        RnsNativePublicPolynomialRoleV1::CiphertextC1
    };
    tail_position_v2(role, Some(record), LEGACY_LIMBS_V2 + within % 2)
}

/// One exact tail object, including the move-only transport receipt.
pub(super) struct RnsNativePublishedTailObjectV2 {
    position: RnsNativeTailSourcePositionV2,
    descriptor: RnsNativePublicPolynomialDescriptorV1,
    encoded_integrity_digest: [u8; 32],
    publication_receipt: ZkAmsMkheDirectObjectPublicationReceiptV1,
}

impl RnsNativePublishedTailObjectV2 {
    fn validate_v2(&self) -> Result<(), RnsNativeTailPublicationErrorV2> {
        let pointer = self.publication_receipt.pointer();
        let expected = RnsNativePublicPolynomialDescriptorV1::new(
            reader_role_v2(self.position),
            self.position.record_ordinal_v2(),
            usize::from(self.position.limb_v2()),
            pointer,
        )
        .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidReceipt)?;
        if self.descriptor != expected
            || self.encoded_integrity_digest == [0; 32]
            || pointer.kind() != self.position.object_kind_v2()
            || usize::try_from(pointer.payload_bytes()).ok() != Some(OBJECT_BYTES_V2)
            || self.publication_receipt.receipt_digest() == [0; 32]
            || self
                .publication_receipt
                .post_publish_read_receipt()
                .snapshot()
                .pointer()
                != pointer
            || self
                .publication_receipt
                .post_publish_read_receipt()
                .canonical_bytes()
                != pointer.payload_bytes()
            || self
                .publication_receipt
                .post_publish_read_receipt()
                .payload_blake3()
                != pointer.payload_blake3()
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
        }
        Ok(())
    }

    pub(super) const fn position_v2(&self) -> RnsNativeTailSourcePositionV2 {
        self.position
    }

    pub(super) const fn descriptor_v2(&self) -> RnsNativePublicPolynomialDescriptorV1 {
        self.descriptor
    }

    pub(super) const fn pointer_v2(&self) -> ZkAmsMkheDirectObjectPointerV1 {
        self.publication_receipt.pointer()
    }

    pub(super) const fn encoded_integrity_digest_v2(&self) -> [u8; 32] {
        self.encoded_integrity_digest
    }

    pub(super) const fn publication_receipt_v2(
        &self,
    ) -> &ZkAmsMkheDirectObjectPublicationReceiptV1 {
        &self.publication_receipt
    }
}

fn publish_tail_object_v2<P>(
    position: RnsNativeTailSourcePositionV2,
    coefficients: &[u64],
    publisher: &mut P,
) -> Result<RnsNativePublishedTailObjectV2, RnsNativeTailPublicationErrorV2>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    position
        .tail_ordinal_v2()
        .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition)?;
    let mut encoder = RnsNativeTailObjectEncoderV2::new_v2(position, coefficients)
        .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
    let mut publication = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
        position.object_kind_v2(),
        OBJECT_BYTES_V2 as u64,
        publisher,
    )
    .map_err(|_| RnsNativeTailPublicationErrorV2::Transport)?;
    let mut prefix = [0_u8; COUNT_PREFIX_BYTES_V2];
    encoder
        .write_count_prefix_v2(&mut prefix)
        .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
    publication
        .write_exact(&prefix)
        .map_err(|_| RnsNativeTailPublicationErrorV2::Transport)?;
    let mut chunk = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for expected_chunk in 0..CHUNKS_PER_OBJECT_V2 {
        let emitted = encoder
            .write_next_chunk_v2(&mut chunk)
            .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
        if usize::from(emitted) != expected_chunk {
            return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
        }
        publication
            .write_exact(&chunk)
            .map_err(|_| RnsNativeTailPublicationErrorV2::Transport)?;
    }
    let encoded: RnsNativeEncodedTailObjectV2 = encoder
        .finish_v2()
        .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
    if encoded.position != position
        || usize::try_from(encoded.encoded_bytes).ok() != Some(OBJECT_BYTES_V2)
        || encoded.encoded_bytes_digest == [0; 32]
        || publication.remaining_bytes() != 0
    {
        return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
    }
    let publication_receipt = publication
        .finish()
        .map_err(|_| RnsNativeTailPublicationErrorV2::Transport)?;
    let descriptor = RnsNativePublicPolynomialDescriptorV1::new(
        reader_role_v2(position),
        position.record_ordinal_v2(),
        usize::from(position.limb_v2()),
        publication_receipt.pointer(),
    )
    .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidReceipt)?;
    let published = RnsNativePublishedTailObjectV2 {
        position,
        descriptor,
        encoded_integrity_digest: encoded.encoded_bytes_digest,
        publication_receipt,
    };
    published.validate_v2()?;
    Ok(published)
}

struct RnsNativeTailCasVisitorV2<'publisher, P: ?Sized> {
    publisher: &'publisher mut P,
    expected: Box<[RnsNativeTailSourcePositionV2]>,
    objects: Vec<RnsNativePublishedTailObjectV2>,
    failure: Option<RnsNativeTailPublicationErrorV2>,
    poisoned: bool,
}

impl<'publisher, P: ?Sized> RnsNativeTailCasVisitorV2<'publisher, P> {
    fn new_v2(
        publisher: &'publisher mut P,
        expected: Box<[RnsNativeTailSourcePositionV2]>,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        let mut objects = Vec::new();
        objects
            .try_reserve_exact(expected.len())
            .map_err(|_| RnsNativeTailPublicationErrorV2::ResourceCeiling)?;
        Ok(Self {
            publisher,
            expected,
            objects,
            failure: None,
            poisoned: false,
        })
    }

    fn finish_v2(
        self,
    ) -> Result<Vec<RnsNativePublishedTailObjectV2>, RnsNativeTailPublicationErrorV2> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if self.poisoned || self.objects.len() != self.expected.len() {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        Ok(self.objects)
    }
}

impl<P> RnsNativeTailCoefficientVisitorV2 for RnsNativeTailCasVisitorV2<'_, P>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    fn visit_tail_coefficients_v2(
        &mut self,
        position: RnsNativeTailSourcePositionV2,
        coefficients: &[u64],
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        if self.poisoned || self.failure.is_some() {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        let Some(expected) = self.expected.get(self.objects.len()).copied() else {
            self.failure = Some(RnsNativeTailPublicationErrorV2::InvalidOrder);
            return Err(RnsNativeBasisExtensionErrorV2::VisitorRejected);
        };
        if position != expected {
            self.failure = Some(RnsNativeTailPublicationErrorV2::InvalidOrder);
            return Err(RnsNativeBasisExtensionErrorV2::VisitorRejected);
        }
        match publish_tail_object_v2(position, coefficients, self.publisher) {
            Ok(object) => self.objects.push(object),
            Err(error) => {
                self.failure = Some(error);
                return Err(RnsNativeBasisExtensionErrorV2::VisitorRejected);
            }
        }
        self.poisoned = false;
        Ok(())
    }
}

pub(super) struct RnsNativePublishedTailRecordV2 {
    record_ordinal: u8,
    sample_index: u64,
    coefficient_digest: [u8; 32],
    objects: Box<[RnsNativePublishedTailObjectV2]>,
}

/// Stateful 4+43*4 publication run. Any fallible transition poisons before
/// entering caller/backend code and cannot be retried.
pub(super) struct RnsNativeTailPublicationLifecycleV2<K, P>
where
    K: ZkAmsMkheDirectObjectCasPublicationV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1,
{
    key_tail: RnsNativePublishedCollectiveKeyTailOwnerV2,
    basis_lifecycle: RnsNativeCiphertextTailLifecycleV2,
    key_publisher: K,
    ciphertext_publisher: P,
    key_objects: Box<[RnsNativePublishedTailObjectV2]>,
    records: Vec<RnsNativePublishedTailRecordV2>,
    next_record: u8,
    poisoned: bool,
}

impl<K, P> RnsNativeTailPublicationLifecycleV2<K, P>
where
    K: ZkAmsMkheDirectObjectCasPublicationV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1,
{
    pub(super) fn begin_v2(
        key_tail: RnsNativeCollectiveKeyTailOwnerV2,
        mut key_publisher: K,
        ciphertext_publisher: P,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        let expected = (0..KEY_TAIL_OBJECTS_V2)
            .map(physical_tail_position_v2)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let mut visitor = RnsNativeTailCasVisitorV2::new_v2(&mut key_publisher, expected)?;
        let key_result = key_tail.publish_key_tail_once_v2(&mut visitor);
        let key_tail = match key_result {
            Ok(key_tail) => key_tail,
            Err(_) => {
                return Err(visitor
                    .failure
                    .unwrap_or(RnsNativeTailPublicationErrorV2::Basis));
            }
        };
        let key_objects = visitor.finish_v2()?.into_boxed_slice();
        if key_objects.len() != KEY_TAIL_OBJECTS_V2 {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        let basis_lifecycle = key_tail
            .begin_ciphertext_tail_lifecycle_v2()
            .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(RECORDS_V2)
            .map_err(|_| RnsNativeTailPublicationErrorV2::ResourceCeiling)?;
        Ok(Self {
            key_tail,
            basis_lifecycle,
            key_publisher,
            ciphertext_publisher,
            key_objects,
            records,
            next_record: 0,
            poisoned: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn publish_next_record_from_v1_callback_v2(
        &mut self,
        workspace: RnsNativeCiphertextTailWorkspaceOwnerV2,
        record_ordinal: u8,
        sample_index: u64,
        canonical_plaintext: &[[u8; 32]],
        ephemeral: &[i64],
        error_zero: &[i64],
        error_one: &[i64],
        encryption_nonce: &[u8; 32],
    ) -> Result<(), RnsNativeTailPublicationErrorV2> {
        if self.poisoned {
            return Err(RnsNativeTailPublicationErrorV2::Poisoned);
        }
        self.poisoned = true;
        if record_ordinal != self.next_record || sample_index != u64::from(record_ordinal) {
            return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
        }
        let opening = self
            .key_tail
            .bind_v1_synchronous_callback_v2(
                workspace,
                record_ordinal,
                sample_index,
                canonical_plaintext,
                ephemeral,
                error_zero,
                error_one,
                encryption_nonce,
            )
            .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
        let physical_start =
            KEY_TAIL_OBJECTS_V2 + usize::from(record_ordinal) * TAIL_OBJECTS_PER_RECORD_V2;
        let expected = (physical_start..physical_start + TAIL_OBJECTS_PER_RECORD_V2)
            .map(physical_tail_position_v2)
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let mut visitor =
            RnsNativeTailCasVisitorV2::new_v2(&mut self.ciphertext_publisher, expected)?;
        let completion_result = self
            .key_tail
            .emit_ciphertext_tail_once_v2(opening, &mut visitor);
        let completion: RnsNativeCiphertextTailCompletionV2 = match completion_result {
            Ok(completion) => completion,
            Err(_) => {
                return Err(visitor
                    .failure
                    .unwrap_or(RnsNativeTailPublicationErrorV2::Basis));
            }
        };
        let objects = visitor.finish_v2()?.into_boxed_slice();
        if completion.record_ordinal_v2() != record_ordinal
            || completion.sample_index_v2() != sample_index
            || usize::from(completion.emitted_limb_count_v2()) != TAIL_OBJECTS_PER_RECORD_V2
            || completion.coefficient_digest_v2() == [0; 32]
            || objects.len() != TAIL_OBJECTS_PER_RECORD_V2
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
        }
        let coefficient_digest = completion.coefficient_digest_v2();
        self.basis_lifecycle
            .accept_record_completion_v2(completion)
            .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
        self.records.push(RnsNativePublishedTailRecordV2 {
            record_ordinal,
            sample_index,
            coefficient_digest,
            objects,
        });
        self.next_record = self
            .next_record
            .checked_add(1)
            .ok_or(RnsNativeTailPublicationErrorV2::ResourceCeiling)?;
        self.poisoned = false;
        Ok(())
    }

    pub(super) fn finish_v2(
        mut self,
    ) -> Result<RnsNativeCompletedTailPublicationV2<K, P>, RnsNativeTailPublicationErrorV2> {
        if self.poisoned {
            return Err(RnsNativeTailPublicationErrorV2::Poisoned);
        }
        self.poisoned = true;
        if usize::from(self.next_record) != RECORDS_V2
            || self.records.len() != RECORDS_V2
            || self.key_objects.len() != KEY_TAIL_OBJECTS_V2
        {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        let aggregate = self
            .basis_lifecycle
            .finish_v2()
            .map_err(|_| RnsNativeTailPublicationErrorV2::Basis)?;
        for (record, owner) in self.records.iter().enumerate() {
            if usize::from(owner.record_ordinal) != record
                || usize::try_from(owner.sample_index).ok() != Some(record)
                || owner.coefficient_digest == [0; 32]
                || owner.objects.len() != TAIL_OBJECTS_PER_RECORD_V2
            {
                return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
            }
        }
        let records = self.records.into_boxed_slice();
        validate_physical_tail_objects_v2(&self.key_objects, &records)?;
        let lifecycle_digest = tail_lifecycle_digest_v2(
            self.key_tail.integrity_digest_v2(),
            &aggregate,
            &self.key_objects,
            &records,
        )?;
        Ok(RnsNativeCompletedTailPublicationV2 {
            receipts: RnsNativeTailPublicationReceiptOwnerV2 {
                key_tail: self.key_tail,
                aggregate,
                key_objects: self.key_objects,
                records,
                lifecycle_digest,
            },
            key_provider: self.key_publisher,
            ciphertext_provider: self.ciphertext_publisher,
        })
    }
}

fn validate_physical_tail_objects_v2(
    key_objects: &[RnsNativePublishedTailObjectV2],
    records: &[RnsNativePublishedTailRecordV2],
) -> Result<(), RnsNativeTailPublicationErrorV2> {
    if key_objects.len() != KEY_TAIL_OBJECTS_V2 || records.len() != RECORDS_V2 {
        return Err(RnsNativeTailPublicationErrorV2::Incomplete);
    }
    validate_physical_tail_object_iter_v2(
        key_objects
            .iter()
            .chain(records.iter().flat_map(|record| record.objects.iter())),
    )
}

fn validate_physical_tail_object_iter_v2<'object>(
    objects: impl IntoIterator<Item = &'object RnsNativePublishedTailObjectV2>,
) -> Result<(), RnsNativeTailPublicationErrorV2> {
    let mut tail_ordinals = BTreeSet::new();
    let mut pointers = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut stages = BTreeSet::new();
    let mut seals = BTreeSet::new();
    let mut publication_receipts = BTreeSet::new();
    let mut read_receipts = BTreeSet::new();
    let mut key_publication_identity = None;
    let mut ciphertext_publication_identity = None;
    let mut object_count = 0;
    for (physical, object) in objects.into_iter().enumerate() {
        object_count = physical + 1;
        object.validate_v2()?;
        if object.position != physical_tail_position_v2(physical)? {
            return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
        }
        let receipt = object.publication_receipt_v2();
        let expected_publication_identity = if physical < KEY_TAIL_OBJECTS_V2 {
            &mut key_publication_identity
        } else {
            &mut ciphertext_publication_identity
        };
        match *expected_publication_identity {
            None => *expected_publication_identity = Some(receipt.publication_identity()),
            Some(expected) if expected == receipt.publication_identity() => {}
            Some(_) => return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt),
        }
        if !tail_ordinals.insert(
            object
                .position
                .tail_ordinal_v2()
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition)?,
        ) || !pointers.insert(object.pointer_v2().pointer_digest())
            || !artifacts.insert(object.descriptor_v2().artifact_digest_v1())
            || !stages.insert((receipt.publication_identity(), receipt.staging_identity()))
            || !seals.insert((receipt.publication_identity(), receipt.seal_identity()))
            || !publication_receipts.insert(receipt.receipt_digest())
            || !read_receipts.insert(receipt.post_publish_read_receipt().receipt_digest())
        {
            return Err(RnsNativeTailPublicationErrorV2::DuplicatePointer);
        }
    }
    if key_publication_identity.is_none()
        || ciphertext_publication_identity.is_none()
        || object_count != TAIL_OBJECTS_V2
        || tail_ordinals.len() != TAIL_OBJECTS_V2
        || pointers.len() != TAIL_OBJECTS_V2
        || artifacts.len() != TAIL_OBJECTS_V2
        || stages.len() != TAIL_OBJECTS_V2
        || seals.len() != TAIL_OBJECTS_V2
        || publication_receipts.len() != TAIL_OBJECTS_V2
        || read_receipts.len() != TAIL_OBJECTS_V2
    {
        return Err(RnsNativeTailPublicationErrorV2::Incomplete);
    }
    Ok(())
}

fn tail_lifecycle_digest_v2(
    key_integrity_digest: [u8; 32],
    aggregate: &RnsNativeCiphertextTailAggregateChecksumV2,
    key_objects: &[RnsNativePublishedTailObjectV2],
    records: &[RnsNativePublishedTailRecordV2],
) -> Result<[u8; 32], RnsNativeTailPublicationErrorV2> {
    tail_lifecycle_digest_from_objects_v2(
        key_integrity_digest,
        aggregate,
        key_objects
            .iter()
            .chain(records.iter().flat_map(|record| record.objects.iter())),
    )
}

fn tail_lifecycle_digest_from_objects_v2<'object>(
    key_integrity_digest: [u8; 32],
    aggregate: &RnsNativeCiphertextTailAggregateChecksumV2,
    objects: impl IntoIterator<Item = &'object RnsNativePublishedTailObjectV2>,
) -> Result<[u8; 32], RnsNativeTailPublicationErrorV2> {
    if key_integrity_digest == [0; 32] || aggregate.completion_digest_v2() == [0; 32] {
        return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
    }
    let mut hash = Keccak256::new();
    hash.update(TAIL_LIFECYCLE_DOMAIN_V2);
    hash.update(&[VERSION_V2]);
    hash.update(&key_integrity_digest);
    hash.update(&aggregate.completion_digest_v2());
    hash.update(&(TAIL_OBJECTS_V2 as u16).to_be_bytes());
    let mut object_count = 0;
    for (physical, object) in objects.into_iter().enumerate() {
        object_count = physical + 1;
        hash.update(&(physical as u16).to_be_bytes());
        hash.update(
            &(object
                .position
                .tail_ordinal_v2()
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition)?
                as u16)
                .to_be_bytes(),
        );
        hash.update(&object.pointer_v2().encode());
        hash.update(&object.descriptor_v2().artifact_digest_v1());
        hash.update(&object.encoded_integrity_digest_v2());
        hash.update(&object.publication_receipt_v2().receipt_digest());
        hash.update(
            &object
                .publication_receipt_v2()
                .post_publish_read_receipt()
                .receipt_digest(),
        );
    }
    if object_count != TAIL_OBJECTS_V2 {
        return Err(RnsNativeTailPublicationErrorV2::Incomplete);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
    }
    Ok(digest)
}

pub(super) struct RnsNativeTailPublicationReceiptOwnerV2 {
    key_tail: RnsNativePublishedCollectiveKeyTailOwnerV2,
    aggregate: RnsNativeCiphertextTailAggregateChecksumV2,
    /// Exact `A[38],A[39],B[38],B[39]` receipt owner.
    key_objects: Box<[RnsNativePublishedTailObjectV2]>,
    /// Exact 43 whole record-local owners, each retaining
    /// `C0[38],C0[39],C1[38],C1[39]` receipts.
    records: Box<[RnsNativePublishedTailRecordV2]>,
    lifecycle_digest: [u8; 32],
}

impl RnsNativeTailPublicationReceiptOwnerV2 {
    fn validate_v2(&self) -> Result<(), RnsNativeTailPublicationErrorV2> {
        validate_physical_tail_objects_v2(&self.key_objects, &self.records)?;
        if self.lifecycle_digest
            != tail_lifecycle_digest_v2(
                self.key_tail.integrity_digest_v2(),
                &self.aggregate,
                &self.key_objects,
                &self.records,
            )?
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
        }
        Ok(())
    }

    pub(super) const fn lifecycle_digest_v2(&self) -> [u8; 32] {
        self.lifecycle_digest
    }
}

pub(super) struct RnsNativeCompletedTailPublicationV2<K, P> {
    receipts: RnsNativeTailPublicationReceiptOwnerV2,
    key_provider: K,
    ciphertext_provider: P,
}

/// Exact future production shape: one whole streaming key authority and every
/// whole record manifest. There is intentionally no constructor in this
/// tranche, so the absent Phase-23/callback bridge cannot be replaced by
/// pointers, copied digests, or independently recreated receipts.
pub(super) struct RnsNativeWholeV1KeyPublicationOwnerV2 {
    key_authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
}

pub(super) struct RnsNativeWholeV1RecordPublicationOwnerV2 {
    record_ordinal: u8,
    ciphertext: ZkAmsMkheStreamingCollectiveCiphertextV1,
}

pub(super) struct RnsNativeWholeV1PublicationOwnersV2 {
    key: RnsNativeWholeV1KeyPublicationOwnerV2,
    records: Box<[RnsNativeWholeV1RecordPublicationOwnerV2]>,
}

impl RnsNativeWholeV1PublicationOwnersV2 {
    fn validate_v1_owner_v2(&self) -> Result<(), RnsNativeTailPublicationErrorV2> {
        if self.key.key_authority.next_sample_index() != RECORDS_V2 as u64
            || self.key.key_authority.public_a_limb_pointers().len() != LEGACY_LIMBS_V2
            || self.key.key_authority.public_b_limb_pointers().len() != LEGACY_LIMBS_V2
            || self.key.key_authority.public_a_publication_receipts().len() != LEGACY_LIMBS_V2
            || self.key.key_authority.public_b_publication_receipts().len() != LEGACY_LIMBS_V2
            || self.records.len() != RECORDS_V2
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidV1Owner);
        }
        for (record, owner) in self.records.iter().enumerate() {
            owner
                .ciphertext
                .validate_for_authority_v1(&self.key.key_authority)
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
            let binding = owner
                .ciphertext
                .sealed_binding_v1()
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
            if usize::from(owner.record_ordinal) != record
                || usize::try_from(owner.ciphertext.sample_index()).ok() != Some(record)
                || owner.ciphertext.constant_limb_pointers().len() != LEGACY_LIMBS_V2
                || owner.ciphertext.linear_limb_pointers().len() != LEGACY_LIMBS_V2
                || binding.constant_publication_receipts().len() != LEGACY_LIMBS_V2
                || binding.linear_publication_receipts().len() != LEGACY_LIMBS_V2
            {
                return Err(RnsNativeTailPublicationErrorV2::InvalidV1Owner);
            }
        }
        Ok(())
    }
}

/// Sole manifest/reader owner for the finalized key: the complete V1
/// authority, its non-replayable arithmetic tail owner, and the four actual
/// `A[38], A[39], B[38], B[39]` publication receipts.
pub(super) struct RnsNativeWholeKeyAndTailPublicationOwnerV2 {
    key_authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    key_tail: RnsNativePublishedCollectiveKeyTailOwnerV2,
    tail_objects: Box<[RnsNativePublishedTailObjectV2]>,
}

/// Sole manifest/reader owner for one finalized record: the complete V1
/// ciphertext and its exact `C0[38], C0[39], C1[38], C1[39]` receipt owner.
pub(super) struct RnsNativeWholeRecordAndTailPublicationOwnerV2 {
    record_ordinal: u8,
    ciphertext: ZkAmsMkheStreamingCollectiveCiphertextV1,
    tail: RnsNativePublishedTailRecordV2,
}

/// All original arithmetic, V1, and publication authorities retained in the
/// exact key-plus-43-record pairing. No digest projection can construct this.
pub(super) struct RnsNativeWholePublicationOwnersV2 {
    key: RnsNativeWholeKeyAndTailPublicationOwnerV2,
    aggregate: RnsNativeCiphertextTailAggregateChecksumV2,
    records: Box<[RnsNativeWholeRecordAndTailPublicationOwnerV2]>,
    lifecycle_digest: [u8; 32],
}

struct RnsNativePublicationUniquenessV2 {
    pointers: BTreeSet<[u8; 32]>,
    stages: BTreeSet<([u8; 32], [u8; 32])>,
    seals: BTreeSet<([u8; 32], [u8; 32])>,
    publication_receipts: BTreeSet<[u8; 32]>,
    read_receipts: BTreeSet<[u8; 32]>,
}

impl RnsNativePublicationUniquenessV2 {
    fn new_v2() -> Self {
        Self {
            pointers: BTreeSet::new(),
            stages: BTreeSet::new(),
            seals: BTreeSet::new(),
            publication_receipts: BTreeSet::new(),
            read_receipts: BTreeSet::new(),
        }
    }

    fn observe_v2(
        &mut self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    ) -> Result<(), RnsNativeTailPublicationErrorV2> {
        let expected_kind = match role {
            RnsNativePublicPolynomialRoleV1::PublicA => {
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA
            }
            RnsNativePublicPolynomialRoleV1::PublicB => {
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB
            }
            RnsNativePublicPolynomialRoleV1::CiphertextC0 => {
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0
            }
            RnsNativePublicPolynomialRoleV1::CiphertextC1 => {
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1
            }
        };
        let requires_record = matches!(
            role,
            RnsNativePublicPolynomialRoleV1::CiphertextC0
                | RnsNativePublicPolynomialRoleV1::CiphertextC1
        );
        if limb >= TARGET_LIMBS_V2
            || requires_record != record.is_some()
            || record.is_some_and(|record| record >= RECORDS_V2)
            || pointer.kind() != expected_kind
            || usize::try_from(pointer.payload_bytes()).ok() != Some(OBJECT_BYTES_V2)
            || ZkAmsMkheDirectObjectPointerV1::decode_exact(expected_kind, &pointer.encode()).ok()
                != Some(pointer)
            || receipt.pointer() != pointer
            || receipt.receipt_digest() == [0; 32]
            || receipt.post_publish_read_receipt().snapshot().pointer() != pointer
            || receipt.post_publish_read_receipt().canonical_bytes() != pointer.payload_bytes()
            || receipt.post_publish_read_receipt().payload_blake3() != pointer.payload_blake3()
            || receipt.post_publish_read_receipt().receipt_digest() == [0; 32]
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
        }
        if !self.pointers.insert(pointer.pointer_digest())
            || !self
                .stages
                .insert((receipt.publication_identity(), receipt.staging_identity()))
            || !self
                .seals
                .insert((receipt.publication_identity(), receipt.seal_identity()))
            || !self.publication_receipts.insert(receipt.receipt_digest())
            || !self
                .read_receipts
                .insert(receipt.post_publish_read_receipt().receipt_digest())
        {
            return Err(RnsNativeTailPublicationErrorV2::DuplicatePointer);
        }
        Ok(())
    }

    fn finish_v2(self) -> Result<(), RnsNativeTailPublicationErrorV2> {
        if self.pointers.len() != FULL_OBJECTS_V2
            || self.stages.len() != FULL_OBJECTS_V2
            || self.seals.len() != FULL_OBJECTS_V2
            || self.publication_receipts.len() != FULL_OBJECTS_V2
            || self.read_receipts.len() != FULL_OBJECTS_V2
        {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        Ok(())
    }
}

impl RnsNativeWholePublicationOwnersV2 {
    fn validate_v2(&self) -> Result<(), RnsNativeTailPublicationErrorV2> {
        if self.key.key_authority.next_sample_index() != RECORDS_V2 as u64
            || self.key.tail_objects.len() != KEY_TAIL_OBJECTS_V2
            || self.records.len() != RECORDS_V2
            || usize::from(self.aggregate.record_count) != RECORDS_V2
            || usize::from(self.aggregate.emitted_limb_count) != CIPHERTEXT_TAIL_OBJECTS_V2
            || self.lifecycle_digest == [0; 32]
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidV1Owner);
        }
        self.key
            .key_tail
            .validate_v1_authority_binding_v2(&self.key.key_authority)
            .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
        for (record, owner) in self.records.iter().enumerate() {
            owner
                .ciphertext
                .validate_for_authority_v1(&self.key.key_authority)
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
            if usize::from(owner.record_ordinal) != record
                || owner.tail.record_ordinal != owner.record_ordinal
                || owner.tail.sample_index != record as u64
                || owner.ciphertext.sample_index() != record as u64
                || owner.tail.coefficient_digest == [0; 32]
                || owner.tail.objects.len() != TAIL_OBJECTS_PER_RECORD_V2
            {
                return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
            }
        }
        let tail_objects = self.key.tail_objects.iter().chain(
            self.records
                .iter()
                .flat_map(|record| record.tail.objects.iter()),
        );
        validate_physical_tail_object_iter_v2(tail_objects)?;
        let lifecycle_digest = tail_lifecycle_digest_from_objects_v2(
            self.key.key_tail.integrity_digest_v2(),
            &self.aggregate,
            self.key.tail_objects.iter().chain(
                self.records
                    .iter()
                    .flat_map(|record| record.tail.objects.iter()),
            ),
        )?;
        if lifecycle_digest != self.lifecycle_digest {
            return Err(RnsNativeTailPublicationErrorV2::InvalidReceipt);
        }

        let mut uniqueness = RnsNativePublicationUniquenessV2::new_v2();
        for (role, pointers, receipts) in [
            (
                RnsNativePublicPolynomialRoleV1::PublicA,
                self.key.key_authority.public_a_limb_pointers(),
                self.key.key_authority.public_a_publication_receipts(),
            ),
            (
                RnsNativePublicPolynomialRoleV1::PublicB,
                self.key.key_authority.public_b_limb_pointers(),
                self.key.key_authority.public_b_publication_receipts(),
            ),
        ] {
            if pointers.len() != LEGACY_LIMBS_V2 || receipts.len() != LEGACY_LIMBS_V2 {
                return Err(RnsNativeTailPublicationErrorV2::InvalidV1Owner);
            }
            for (limb, (pointer, receipt)) in pointers.iter().zip(receipts).enumerate() {
                uniqueness.observe_v2(role, None, limb, *pointer, receipt)?;
            }
        }
        for (record, owner) in self.records.iter().enumerate() {
            let binding = owner
                .ciphertext
                .sealed_binding_v1()
                .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
            for (role, pointers, receipts) in [
                (
                    RnsNativePublicPolynomialRoleV1::CiphertextC0,
                    binding.constant_limb_pointers(),
                    binding.constant_publication_receipts(),
                ),
                (
                    RnsNativePublicPolynomialRoleV1::CiphertextC1,
                    binding.linear_limb_pointers(),
                    binding.linear_publication_receipts(),
                ),
            ] {
                if pointers.len() != LEGACY_LIMBS_V2 || receipts.len() != LEGACY_LIMBS_V2 {
                    return Err(RnsNativeTailPublicationErrorV2::InvalidV1Owner);
                }
                for (limb, (pointer, receipt)) in pointers.iter().zip(receipts).enumerate() {
                    uniqueness.observe_v2(role, Some(record), limb, *pointer, receipt)?;
                }
            }
        }
        for object in self.key.tail_objects.iter().chain(
            self.records
                .iter()
                .flat_map(|record| record.tail.objects.iter()),
        ) {
            let position = object.position_v2();
            uniqueness.observe_v2(
                reader_role_v2(position),
                position.record_ordinal_v2().map(usize::from),
                usize::from(position.limb_v2()),
                object.pointer_v2(),
                object.publication_receipt_v2(),
            )?;
        }
        uniqueness.finish_v2()
    }

    fn prefix_pointer_v2(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Option<ZkAmsMkheDirectObjectPointerV1> {
        if limb >= LEGACY_LIMBS_V2 {
            return None;
        }
        match (role, record) {
            (RnsNativePublicPolynomialRoleV1::PublicA, None) => self
                .key
                .key_authority
                .public_a_limb_pointers()
                .get(limb)
                .copied(),
            (RnsNativePublicPolynomialRoleV1::PublicB, None) => self
                .key
                .key_authority
                .public_b_limb_pointers()
                .get(limb)
                .copied(),
            (RnsNativePublicPolynomialRoleV1::CiphertextC0, Some(record)) => self
                .records
                .get(record)?
                .ciphertext
                .constant_limb_pointers()
                .get(limb)
                .copied(),
            (RnsNativePublicPolynomialRoleV1::CiphertextC1, Some(record)) => self
                .records
                .get(record)?
                .ciphertext
                .linear_limb_pointers()
                .get(limb)
                .copied(),
            _ => None,
        }
    }

    fn tail_object_for_position_v2(
        &self,
        position: RnsNativeTailSourcePositionV2,
    ) -> Option<&RnsNativePublishedTailObjectV2> {
        self.key
            .tail_objects
            .iter()
            .chain(
                self.records
                    .iter()
                    .flat_map(|record| record.tail.objects.iter()),
            )
            .find(|object| object.position == position)
    }
}

fn pair_whole_publication_owners_v2(
    v1: RnsNativeWholeV1PublicationOwnersV2,
    tails: RnsNativeTailPublicationReceiptOwnerV2,
) -> Result<RnsNativeWholePublicationOwnersV2, RnsNativeTailPublicationErrorV2> {
    v1.validate_v1_owner_v2()?;
    tails.validate_v2()?;
    tails
        .key_tail
        .validate_v1_authority_binding_v2(&v1.key.key_authority)
        .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;

    let RnsNativeWholeV1PublicationOwnersV2 { key, records } = v1;
    let RnsNativeTailPublicationReceiptOwnerV2 {
        key_tail,
        aggregate,
        key_objects,
        records: tail_records,
        lifecycle_digest,
    } = tails;
    let mut paired_records = Vec::new();
    paired_records
        .try_reserve_exact(RECORDS_V2)
        .map_err(|_| RnsNativeTailPublicationErrorV2::ResourceCeiling)?;
    for (v1_record, tail) in records.into_vec().into_iter().zip(tail_records.into_vec()) {
        if v1_record.record_ordinal != tail.record_ordinal
            || v1_record.ciphertext.sample_index() != tail.sample_index
        {
            return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
        }
        paired_records.push(RnsNativeWholeRecordAndTailPublicationOwnerV2 {
            record_ordinal: v1_record.record_ordinal,
            ciphertext: v1_record.ciphertext,
            tail,
        });
    }
    let owners = RnsNativeWholePublicationOwnersV2 {
        key: RnsNativeWholeKeyAndTailPublicationOwnerV2 {
            key_authority: key.key_authority,
            key_tail,
            tail_objects: key_objects,
        },
        aggregate,
        records: paired_records.into_boxed_slice(),
        lifecycle_digest,
    };
    owners.validate_v2()?;
    Ok(owners)
}

/// Explicitly uninhabited production adapter. A future live delta must consume
/// the Phase-23 owner and the synchronous callback rather than add a raw
/// constructor to `RnsNativeWholeV1PublicationOwnersV2`.
pub(super) struct RnsNativeWholeV1ProductionAdapterV2 {
    never: Infallible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RnsNativeCompositeRouteV2 {
    Key,
    Ciphertext,
}

#[derive(Clone, Copy)]
struct RnsNativeAllowlistedPointerV2 {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    route: RnsNativeCompositeRouteV2,
}

/// Move-only routed provider over the full typed 3,520-pointer inventory.
pub(super) struct RnsNativeTailCompositeProviderV2<K, P> {
    key_provider: K,
    ciphertext_provider: P,
    allowlist: BTreeMap<[u8; 32], RnsNativeAllowlistedPointerV2>,
    key_provider_identity: [u8; 32],
    key_snapshot_identity: [u8; 32],
    ciphertext_provider_identity: [u8; 32],
    ciphertext_snapshot_identity: [u8; 32],
    composite_provider_identity: [u8; 32],
    composite_snapshot_identity: [u8; 32],
}

impl<K, P> RnsNativeTailCompositeProviderV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    fn new_v2(
        key_provider: K,
        ciphertext_provider: P,
        pointers: Box<[RnsNativeAllowlistedPointerV2]>,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        Self::new_with_expected_inventory_v2(
            key_provider,
            ciphertext_provider,
            pointers,
            FULL_OBJECTS_V2,
            true,
        )
    }

    #[cfg(test)]
    fn new_bounded_test_v2(
        key_provider: K,
        ciphertext_provider: P,
        pointers: Box<[RnsNativeAllowlistedPointerV2]>,
        expected_pointer_count: usize,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        Self::new_with_expected_inventory_v2(
            key_provider,
            ciphertext_provider,
            pointers,
            expected_pointer_count,
            false,
        )
    }

    fn new_with_expected_inventory_v2(
        mut key_provider: K,
        mut ciphertext_provider: P,
        pointers: Box<[RnsNativeAllowlistedPointerV2]>,
        expected_pointer_count: usize,
        require_full_canonical_order: bool,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        if expected_pointer_count == 0 || pointers.len() != expected_pointer_count {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        let key_provider_identity = key_provider
            .provider_identity()
            .map_err(|_| RnsNativeTailPublicationErrorV2::ProviderAxes)?;
        let key_snapshot_identity = key_provider
            .snapshot_identity()
            .map_err(|_| RnsNativeTailPublicationErrorV2::ProviderAxes)?;
        let ciphertext_provider_identity = ciphertext_provider
            .provider_identity()
            .map_err(|_| RnsNativeTailPublicationErrorV2::ProviderAxes)?;
        let ciphertext_snapshot_identity = ciphertext_provider
            .snapshot_identity()
            .map_err(|_| RnsNativeTailPublicationErrorV2::ProviderAxes)?;
        if [
            key_provider_identity,
            key_snapshot_identity,
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
        ]
        .contains(&[0; 32])
        {
            return Err(RnsNativeTailPublicationErrorV2::ProviderAxes);
        }
        let mut allowlist = BTreeMap::new();
        for (ordinal, entry) in pointers.iter().copied().enumerate() {
            if require_full_canonical_order
                && expected_full_manifest_kind_v2(ordinal) != Some(entry.pointer.kind())
            {
                return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
            }
            let expected_route = match entry.pointer.kind() {
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA
                | ZkAmsMkheDirectObjectKindV1::CollectivePublicB => RnsNativeCompositeRouteV2::Key,
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0
                | ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1 => {
                    RnsNativeCompositeRouteV2::Ciphertext
                }
                _ => return Err(RnsNativeTailPublicationErrorV2::WrongRoute),
            };
            if entry.route != expected_route {
                return Err(RnsNativeTailPublicationErrorV2::WrongRoute);
            }
            if usize::try_from(entry.pointer.payload_bytes()).ok() != Some(OBJECT_BYTES_V2) {
                return Err(RnsNativeTailPublicationErrorV2::LengthMutation);
            }
            if allowlist
                .insert(entry.pointer.pointer_digest(), entry)
                .is_some()
            {
                return Err(RnsNativeTailPublicationErrorV2::DuplicatePointer);
            }
        }
        let composite_provider_identity = composite_axes_digest_v2(
            COMPOSITE_PROVIDER_IDENTITY_DOMAIN_V2,
            key_provider_identity,
            ciphertext_provider_identity,
            &pointers,
        );
        let composite_snapshot_identity = composite_axes_digest_v2(
            COMPOSITE_SNAPSHOT_IDENTITY_DOMAIN_V2,
            key_snapshot_identity,
            ciphertext_snapshot_identity,
            &pointers,
        );
        if composite_provider_identity == [0; 32]
            || composite_snapshot_identity == [0; 32]
            || composite_provider_identity == composite_snapshot_identity
        {
            return Err(RnsNativeTailPublicationErrorV2::ProviderAxes);
        }
        let mut provider = Self {
            key_provider,
            ciphertext_provider,
            allowlist,
            key_provider_identity,
            key_snapshot_identity,
            ciphertext_provider_identity,
            ciphertext_snapshot_identity,
            composite_provider_identity,
            composite_snapshot_identity,
        };
        provider
            .ensure_axes_v2()
            .map_err(|_| RnsNativeTailPublicationErrorV2::ProviderAxes)?;
        Ok(provider)
    }

    fn ensure_axes_v2(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        let key_provider_identity = self.key_provider.provider_identity()?;
        let key_snapshot_identity = self.key_provider.snapshot_identity()?;
        let ciphertext_provider_identity = self.ciphertext_provider.provider_identity()?;
        let ciphertext_snapshot_identity = self.ciphertext_provider.snapshot_identity()?;
        if key_provider_identity != self.key_provider_identity
            || key_snapshot_identity != self.key_snapshot_identity
            || ciphertext_provider_identity != self.ciphertext_provider_identity
            || ciphertext_snapshot_identity != self.ciphertext_snapshot_identity
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn allowlisted_v2(
        &self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<RnsNativeAllowlistedPointerV2, ZkAmsMkheErrorV1> {
        let entry = self
            .allowlist
            .get(&pointer.pointer_digest())
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if entry.pointer != pointer {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(entry)
    }
}

fn expected_full_manifest_kind_v2(ordinal: usize) -> Option<ZkAmsMkheDirectObjectKindV1> {
    if ordinal < TARGET_LIMBS_V2 {
        Some(ZkAmsMkheDirectObjectKindV1::CollectivePublicA)
    } else if ordinal < 2 * TARGET_LIMBS_V2 {
        Some(ZkAmsMkheDirectObjectKindV1::CollectivePublicB)
    } else if ordinal < 2 * TARGET_LIMBS_V2 + RECORDS_V2 * TARGET_LIMBS_V2 {
        Some(ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0)
    } else if ordinal < FULL_OBJECTS_V2 {
        Some(ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1)
    } else {
        None
    }
}

fn composite_axes_digest_v2(
    domain: &'static [u8],
    key_axis: [u8; 32],
    ciphertext_axis: [u8; 32],
    pointers: &[RnsNativeAllowlistedPointerV2],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[VERSION_V2]);
    hash.update(&key_axis);
    hash.update(&ciphertext_axis);
    hash.update(&(pointers.len() as u16).to_be_bytes());
    for entry in pointers {
        hash.update(&[match entry.route {
            RnsNativeCompositeRouteV2::Key => 0,
            RnsNativeCompositeRouteV2::Ciphertext => 1,
        }]);
        hash.update(&entry.pointer.encode());
    }
    hash.finalize()
}

impl<K, P> ZkAmsMkheDirectObjectReadAtProviderV1 for RnsNativeTailCompositeProviderV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.ensure_axes_v2()?;
        Ok(self.composite_provider_identity)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.ensure_axes_v2()?;
        Ok(self.composite_snapshot_identity)
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let entry = self.allowlisted_v2(pointer)?;
        self.ensure_axes_v2()?;
        let length = match entry.route {
            RnsNativeCompositeRouteV2::Key => self.key_provider.object_len(pointer),
            RnsNativeCompositeRouteV2::Ciphertext => self.ciphertext_provider.object_len(pointer),
        }?;
        self.ensure_axes_v2()?;
        if length != pointer.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(length)
    }

    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        let entry = self.allowlisted_v2(pointer)?;
        let requested = u64::try_from(destination.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if destination.is_empty()
            || destination.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1
            || absolute_offset
                .checked_add(requested)
                .is_none_or(|end| end > pointer.payload_bytes())
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.ensure_axes_v2()?;
        let read = match entry.route {
            RnsNativeCompositeRouteV2::Key => {
                self.key_provider
                    .read_at(pointer, absolute_offset, destination)
            }
            RnsNativeCompositeRouteV2::Ciphertext => {
                self.ciphertext_provider
                    .read_at(pointer, absolute_offset, destination)
            }
        }?;
        self.ensure_axes_v2()?;
        if read != destination.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(read)
    }
}

pub(super) struct RnsNativeExistingReaderBridgeV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    reader: RnsNativePublicPolynomialReaderV1<RnsNativeTailCompositeProviderV2<K, P>>,
    owners: RnsNativeWholePublicationOwnersV2,
}

impl<K, P> RnsNativeCompletedTailPublicationV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    pub(super) fn into_existing_reader_bridge_v2(
        self,
        v1: RnsNativeWholeV1PublicationOwnersV2,
    ) -> Result<RnsNativeExistingReaderBridgeV2<K, P>, RnsNativeTailPublicationErrorV2> {
        let owners = pair_whole_publication_owners_v2(v1, self.receipts)?;
        // Pairing and full 3,520-object receipt uniqueness are validated before
        // any prefix/full-manifest descriptor list or allow-list is built.
        let (manifest, pointers) = build_existing_manifest_v2(&owners)?;
        let provider = RnsNativeTailCompositeProviderV2::new_v2(
            self.key_provider,
            self.ciphertext_provider,
            pointers,
        )?;
        let reader = RnsNativePublicPolynomialReaderV1::new(manifest, provider)
            .map_err(|_| RnsNativeTailPublicationErrorV2::Reader)?;
        Ok(RnsNativeExistingReaderBridgeV2 { reader, owners })
    }
}

fn descriptor_v2(
    owners: &RnsNativeWholePublicationOwnersV2,
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<usize>,
    limb: usize,
) -> Result<RnsNativePublicPolynomialDescriptorV1, RnsNativeTailPublicationErrorV2> {
    let reader_record = record
        .map(u8::try_from)
        .transpose()
        .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidPosition)?;
    if limb < LEGACY_LIMBS_V2 {
        let pointer = owners
            .prefix_pointer_v2(role, record, limb)
            .ok_or(RnsNativeTailPublicationErrorV2::InvalidV1Owner)?;
        return RnsNativePublicPolynomialDescriptorV1::new(role, reader_record, limb, pointer)
            .map_err(|_| RnsNativeTailPublicationErrorV2::InvalidV1Owner);
    }
    let position = tail_position_v2(role, record, limb)?;
    let object = owners
        .tail_object_for_position_v2(position)
        .ok_or(RnsNativeTailPublicationErrorV2::Incomplete)?;
    object.validate_v2()?;
    Ok(object.descriptor_v2())
}

fn build_existing_manifest_v2(
    owners: &RnsNativeWholePublicationOwnersV2,
) -> Result<
    (
        RnsNativePublicPolynomialManifestV1,
        Box<[RnsNativeAllowlistedPointerV2]>,
    ),
    RnsNativeTailPublicationErrorV2,
> {
    let mut public_a = Vec::with_capacity(TARGET_LIMBS_V2);
    let mut public_b = Vec::with_capacity(TARGET_LIMBS_V2);
    let mut ciphertext_c0 = Vec::with_capacity(RECORDS_V2 * TARGET_LIMBS_V2);
    let mut ciphertext_c1 = Vec::with_capacity(RECORDS_V2 * TARGET_LIMBS_V2);
    for limb in 0..TARGET_LIMBS_V2 {
        public_a.push(descriptor_v2(
            owners,
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            limb,
        )?);
        public_b.push(descriptor_v2(
            owners,
            RnsNativePublicPolynomialRoleV1::PublicB,
            None,
            limb,
        )?);
    }
    for record in 0..RECORDS_V2 {
        for limb in 0..TARGET_LIMBS_V2 {
            ciphertext_c0.push(descriptor_v2(
                owners,
                RnsNativePublicPolynomialRoleV1::CiphertextC0,
                Some(record),
                limb,
            )?);
            ciphertext_c1.push(descriptor_v2(
                owners,
                RnsNativePublicPolynomialRoleV1::CiphertextC1,
                Some(record),
                limb,
            )?);
        }
    }
    let mut pointers = Vec::with_capacity(FULL_OBJECTS_V2);
    for descriptor in public_a
        .iter()
        .chain(public_b.iter())
        .chain(ciphertext_c0.iter())
        .chain(ciphertext_c1.iter())
    {
        let pointer = descriptor.pointer_v1();
        let route = match pointer.kind() {
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA
            | ZkAmsMkheDirectObjectKindV1::CollectivePublicB => RnsNativeCompositeRouteV2::Key,
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0
            | ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1 => {
                RnsNativeCompositeRouteV2::Ciphertext
            }
            _ => return Err(RnsNativeTailPublicationErrorV2::WrongRoute),
        };
        pointers.push(RnsNativeAllowlistedPointerV2 { pointer, route });
    }
    if pointers.len() != FULL_OBJECTS_V2 {
        return Err(RnsNativeTailPublicationErrorV2::Incomplete);
    }
    let manifest = RnsNativePublicPolynomialManifestV1::new(
        public_a.into_boxed_slice(),
        public_b.into_boxed_slice(),
        ciphertext_c0.into_boxed_slice(),
        ciphertext_c1.into_boxed_slice(),
    )
    .map_err(|_| RnsNativeTailPublicationErrorV2::Reader)?;
    Ok((manifest, pointers.into_boxed_slice()))
}

/// One owned relation schedule threaded through exactly 40*5 reader outputs.
pub(super) struct RnsNativeSingleQpcsScheduleBatchV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    bridge: RnsNativeExistingReaderBridgeV2<K, P>,
    schedule: Option<RnsNativeQpcsRelationScheduleV1>,
    evaluations: Vec<RnsNativePublicPolynomialEvaluationV1>,
    next_evaluation: usize,
    poisoned: bool,
}

impl<K, P> RnsNativeSingleQpcsScheduleBatchV2<K, P>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    pub(super) fn begin_v2(
        bridge: RnsNativeExistingReaderBridgeV2<K, P>,
        schedule: RnsNativeQpcsRelationScheduleV1,
    ) -> Result<Self, RnsNativeTailPublicationErrorV2> {
        let mut evaluations = Vec::new();
        evaluations
            .try_reserve_exact(QPCS_EVALUATIONS_V2)
            .map_err(|_| RnsNativeTailPublicationErrorV2::ResourceCeiling)?;
        Ok(Self {
            bridge,
            schedule: Some(schedule),
            evaluations,
            next_evaluation: 0,
            poisoned: false,
        })
    }

    pub(super) fn take_next_evaluation_v2(
        &mut self,
    ) -> Result<RnsNativePublicPolynomialEvaluationV1, RnsNativeTailPublicationErrorV2> {
        if self.poisoned {
            return Err(RnsNativeTailPublicationErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.next_evaluation >= QPCS_EVALUATIONS_V2 {
            return Err(RnsNativeTailPublicationErrorV2::InvalidOrder);
        }
        let limb = self.next_evaluation / QPCS_REPETITIONS_V2;
        let repetition = self.next_evaluation % QPCS_REPETITIONS_V2;
        let schedule = self
            .schedule
            .as_ref()
            .ok_or(RnsNativeTailPublicationErrorV2::Incomplete)?;
        let evaluation = self
            .bridge
            .reader
            .take_next_evaluation_v1(schedule, limb, repetition)
            .map_err(|_| RnsNativeTailPublicationErrorV2::Reader)?;
        self.evaluations.push(evaluation);
        self.next_evaluation += 1;
        self.poisoned = false;
        Ok(evaluation)
    }

    pub(super) fn finish_v2(
        mut self,
    ) -> Result<RnsNativeCompletedQpcsSourceReadV2, RnsNativeTailPublicationErrorV2> {
        if self.poisoned {
            return Err(RnsNativeTailPublicationErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.next_evaluation != QPCS_EVALUATIONS_V2
            || self.evaluations.len() != QPCS_EVALUATIONS_V2
        {
            return Err(RnsNativeTailPublicationErrorV2::Incomplete);
        }
        let RnsNativeExistingReaderBridgeV2 { reader, owners } = self.bridge;
        let read_receipt = reader
            .finish()
            .map_err(|_| RnsNativeTailPublicationErrorV2::Reader)?;
        let schedule = self
            .schedule
            .take()
            .ok_or(RnsNativeTailPublicationErrorV2::Incomplete)?;
        Ok(RnsNativeCompletedQpcsSourceReadV2 {
            owners,
            schedule,
            evaluations: self.evaluations.into_boxed_slice(),
            read_receipt,
        })
    }
}

/// Exact complete public read plus all 200 ordered values, the same move-only
/// qPCS schedule, and every underlying V1/tail authority owner.
pub(super) struct RnsNativeCompletedQpcsSourceReadV2 {
    owners: RnsNativeWholePublicationOwnersV2,
    schedule: RnsNativeQpcsRelationScheduleV1,
    evaluations: Box<[RnsNativePublicPolynomialEvaluationV1]>,
    read_receipt: RnsNativePublicPolynomialReadReceiptV1,
}

impl RnsNativeCompletedQpcsSourceReadV2 {
    pub(super) const fn schedule_v2(&self) -> &RnsNativeQpcsRelationScheduleV1 {
        &self.schedule
    }

    pub(super) fn evaluations_v2(&self) -> &[RnsNativePublicPolynomialEvaluationV1] {
        &self.evaluations
    }

    pub(super) const fn read_receipt_v2(&self) -> &RnsNativePublicPolynomialReadReceiptV1 {
        &self.read_receipt
    }
}

/// Existing-reader failures are intentionally collapsed at this bridge; the
/// consumed authority owners and providers are destroyed together.
fn _reader_error_is_closed_v2(_: RnsNativePublicPolynomialReaderErrorV1) {}

#[path = "incremental_source_rns_native_tail_publication_v2/pretranscript_public_statement_v2.rs"]
mod pretranscript_public_statement_v2;

#[path = "incremental_source_rns_native_tail_publication_v2/numeric_opening_handoff_v2.rs"]
mod numeric_opening_handoff_v2;

#[cfg(test)]
#[path = "incremental_source_rns_native_tail_publication_v2_tests.rs"]
mod tests;
