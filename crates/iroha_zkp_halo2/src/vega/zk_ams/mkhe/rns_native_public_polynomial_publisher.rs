//! Fail-closed 40-limb public-polynomial publication contract.
//!
//! The live collective path cannot implement this contract yet. Its governed
//! release profile has 38 moduli; it publishes 38 limbs of common `A`, 38 limbs
//! of aggregate `B`, and, for each of the 43 Phase-23 records, 38 limbs of
//! `C0` and 38 limbs of `C1`. The retained Phase-23 owner contains only those
//! pointers and publication receipts and explicitly retains no native
//! ciphertext. Consequently the 176 objects at limbs 38 and 39 do not exist.
//! A digest of a 38-limb object set is not coefficient material and is never
//! accepted as a source for the two missing limbs.
//!
//! This private tranche fixes the source order, canonical coefficient-domain
//! encoder, authenticated CAS publication sequence, manifest construction, and
//! typed reader handoff. The production adapter is deliberately uninhabited.
//! Declaring this file must not be interpreted as creating the missing source
//! owner or closing any integration, readiness, admission, or release gate.

#![allow(
    dead_code,
    reason = "private fail-closed publication contract awaits a real 40-limb coefficient owner"
)]

use std::sync::OnceLock;

use super::{
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1,
        ZkAmsMkheDirectObjectCasPublicationV1, ZkAmsMkheDirectObjectKindV1,
        ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectPublicationReceiptV1,
        ZkAmsMkheDirectObjectPublicationTransactionV1, ZkAmsMkheDirectObjectReadAtProviderV1,
    },
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1,
    },
    rns_native_public_polynomial_reader::{
        RnsNativePublicPolynomialDescriptorV1, RnsNativePublicPolynomialEvaluationV1,
        RnsNativePublicPolynomialManifestV1, RnsNativePublicPolynomialReadReceiptV1,
        RnsNativePublicPolynomialReaderErrorV1, RnsNativePublicPolynomialReaderV1,
        RnsNativePublicPolynomialRoleV1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
};
use crate::vega::sponge::Keccak256;

const VERSION_V1: u8 = 1;
const DIGEST_BYTES_V1: usize = 32;
const RECORDS_V1: usize = 43;
const LEGACY_RELEASE_LIMBS_V1: usize = 38;
const PUBLIC_KEY_ROLES_V1: usize = 2;
const CIPHERTEXT_ROLES_V1: usize = 2;
const ROLES_PER_RECORD_SET_V1: usize = PUBLIC_KEY_ROLES_V1 + CIPHERTEXT_ROLES_V1 * RECORDS_V1;
const OBJECTS_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * ROLES_PER_RECORD_SET_V1;
const LEGACY_OBJECTS_V1: usize = LEGACY_RELEASE_LIMBS_V1 * ROLES_PER_RECORD_SET_V1;
const MISSING_NEW_LIMB_OBJECTS_V1: usize = OBJECTS_V1 - LEGACY_OBJECTS_V1;
const CIPHERTEXT_OBJECTS_PER_COMPONENT_V1: usize = RECORDS_V1 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
const COUNT_PREFIX_BYTES_V1: usize = core::mem::size_of::<u32>();
const RESIDUE_BYTES_V1: usize = core::mem::size_of::<u64>();
const COEFFICIENTS_PER_CHUNK_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / RESIDUE_BYTES_V1;
const CHUNKS_PER_OBJECT_V1: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / COEFFICIENTS_PER_CHUNK_V1;
const OBJECT_BYTES_V1: u64 =
    (COUNT_PREFIX_BYTES_V1 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1) as u64;
const WRITE_CALLS_PER_OBJECT_V1: usize = 1 + CHUNKS_PER_OBJECT_V1;
const SOURCE_CHUNK_CALLS_V1: usize = OBJECTS_V1 * CHUNKS_PER_OBJECT_V1;
const PUBLICATION_WRITE_CALLS_V1: usize = OBJECTS_V1 * WRITE_CALLS_PER_OBJECT_V1;
const PUBLICATION_TRANSPORT_CALLS_V1: usize = 3 * PUBLICATION_WRITE_CALLS_V1;
const CANONICAL_COEFFICIENTS_V1: u64 =
    OBJECTS_V1 as u64 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64;
const CANONICAL_BYTES_V1: u64 = OBJECTS_V1 as u64 * OBJECT_BYTES_V1;
// Each canonical byte crosses the source-to-stage write, immutable-seal
// reread, and post-publication provider readback exactly once.
const AUTHENTICATED_TRANSFER_BYTES_V1: u64 = 3 * CANONICAL_BYTES_V1;
const COARSE_WORK_UNITS_V1: u64 = AUTHENTICATED_TRANSFER_BYTES_V1 + CANONICAL_COEFFICIENTS_V1;
const POINTER_FRAME_BYTES_V1: usize = OBJECTS_V1 * ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1;
const SOURCE_CHUNK_WORKSPACE_BYTES_V1: usize = COEFFICIENTS_PER_CHUNK_V1 * RESIDUE_BYTES_V1;
const ENCODER_CHUNK_WORKSPACE_BYTES_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1;
const TRANSPORT_CHUNK_WORKSPACE_BYTES_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1;
const PUBLICATION_STACK_WORKSPACE_BYTES_V1: usize = SOURCE_CHUNK_WORKSPACE_BYTES_V1
    + ENCODER_CHUNK_WORKSPACE_BYTES_V1
    + TRANSPORT_CHUNK_WORKSPACE_BYTES_V1;
const PUBLICATION_RESOURCE_ACCOUNTING_SCOPE_V1: &[u8] = b"includes-source-to-stage-canonical-bytes;includes-immutable-seal-complete-reread;includes-post-publication-provider-complete-readback;includes-one-canonicality-check-per-residue;excludes-later-public-polynomial-reader-io-and-modular-evaluation-work;excludes-cas-backend-storage;excludes-allocator-overhead;excludes-measured-rss-and-device-evidence";

const ENCODING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publisher-encoding";
const POSITION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publisher-position";
const SOURCE_CONTRACT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publisher-source-contract";
const SOURCE_STREAM_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publisher-source-stream";
const SOURCE_TERMINAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publisher-source-terminal";
const PUBLICATION_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.publication-set";
const READER_HANDOFF_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.reader-handoff";
const PUBLISHED_READ_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.published-read";
const ENCODING_LANGUAGE_V1: &[u8] = b"one-object-per-role-record-limb;coefficient-domain-only;u32-count-131072-big-endian;then-131072-canonical-u64-big-endian-residues;ascending-coefficient-order;strict-less-than-position-modulus;no-reduction;no-ntt-order;8192-byte-coefficient-chunks";
const ORDER_LANGUAGE_V1: &[u8] = b"A-limb-0-through-39;then-B-limb-0-through-39;then-C0-record-major-0-through-42-each-limb-0-through-39;then-C1-record-major-0-through-42-each-limb-0-through-39;exactly-3520-distinct-pointers";
const SOURCE_LANGUAGE_V1: &[u8] = b"move-only-source;nonzero-stable-source-identity-before-and-after-every-chunk;exact-request-digest;128-exact-1024-residue-chunks-per-object;complete-terminal-seal-after-3520-objects-and-461373440-residues;terminal-binds-nonzero-distinct-governed-upstream-owner-digest;legacy-38-digests-and-fabricated-limbs-38-39-forbidden";

/// Exact upstream work still required to inhabit the production adapter.
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_REMAINING_DELTA_V1: &[u8] = b"introduce-a-governed-40-modulus-collective-key-owner;compute-and-retain-coefficient-domain-A-and-B-for-limbs-0-through-39;encrypt-all-43-phase23-records-under-the-same-40-modulus-profile;retain-or-stream-each-coefficient-domain-C0-and-C1-before-native-ownership-is-dropped;produce-upstream-40-limb-positive-and-negative-KATs;prove-transition-from-the-legacy-38-limb-release-profile-without-inferring-limbs-38-and-39;inhabit-the-production-source-adapter;integrate-this-module;qualify-aggregate-io-work-rss-and-device-evidence;keep-composite-readiness-and-release-false-until-all-evidence-passes";

pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_SOURCE_CONTRACT_SETTLED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_ENCODER_SETTLED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_HANDOFF_SETTLED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_DECLARED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_OWNER_AVAILABLE_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_ADAPTER_INHABITED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_INTEGRATED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_READINESS_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_RELEASE_GATE_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_LATER_READER_RESOURCE_EVIDENCE_INCLUDED_V1: bool =
    false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_EVIDENCE_INCLUDED_V1: bool = false;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(LEGACY_RELEASE_LIMBS_V1 == 38);
    assert!(RECORDS_V1 == 43);
    assert!(ROLES_PER_RECORD_SET_V1 == 88);
    assert!(OBJECTS_V1 == 3_520);
    assert!(LEGACY_OBJECTS_V1 == 3_344);
    assert!(MISSING_NEW_LIMB_OBJECTS_V1 == 176);
    assert!(CIPHERTEXT_OBJECTS_PER_COMPONENT_V1 == 1_720);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(COUNT_PREFIX_BYTES_V1 == 4);
    assert!(COEFFICIENTS_PER_CHUNK_V1 == 1_024);
    assert!(CHUNKS_PER_OBJECT_V1 == 128);
    assert!(OBJECT_BYTES_V1 == 1_048_580);
    assert!(WRITE_CALLS_PER_OBJECT_V1 == 129);
    assert!(SOURCE_CHUNK_CALLS_V1 == 450_560);
    assert!(PUBLICATION_WRITE_CALLS_V1 == 454_080);
    assert!(PUBLICATION_TRANSPORT_CALLS_V1 == 1_362_240);
    assert!(CANONICAL_COEFFICIENTS_V1 == 461_373_440);
    assert!(CANONICAL_BYTES_V1 == 3_691_001_600);
    assert!(AUTHENTICATED_TRANSFER_BYTES_V1 == 11_073_004_800);
    assert!(COARSE_WORK_UNITS_V1 == 11_534_378_240);
    assert!(POINTER_FRAME_BYTES_V1 == 274_560);
    assert!(PUBLICATION_STACK_WORKSPACE_BYTES_V1 == 24_576);
    assert!(AUTHENTICATED_TRANSFER_BYTES_V1 < ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1);
    assert!(COARSE_WORK_UNITS_V1 < ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_SOURCE_CONTRACT_SETTLED_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_ENCODER_SETTLED_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_HANDOFF_SETTLED_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_DECLARED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_OWNER_AVAILABLE_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_ADAPTER_INHABITED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_READINESS_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PUBLISHER_RELEASE_GATE_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_LATER_READER_RESOURCE_EVIDENCE_INCLUDED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_EVIDENCE_INCLUDED_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativePublicPolynomialPublisherErrorV1 {
    InvalidSource,
    InvalidPosition,
    InvalidOrder,
    InvalidCoefficient,
    InvalidPublication,
    DuplicatePointer,
    ResourceCeilingExceeded,
    ArithmeticOverflow,
    Incomplete,
    ReaderHandoff,
}

impl core::fmt::Display for RnsNativePublicPolynomialPublisherErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativePublicPolynomialPublisherErrorV1 {}

fn encoding_contract_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    static DIGEST: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    *DIGEST.get_or_init(|| {
        let mut hash = Keccak256::new();
        hash.update(ENCODING_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
        hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
        hash.update(&(RECORDS_V1 as u16).to_be_bytes());
        hash.update(&OBJECT_BYTES_V1.to_be_bytes());
        hash.update(&(COEFFICIENTS_PER_CHUNK_V1 as u16).to_be_bytes());
        hash.update(&(ENCODING_LANGUAGE_V1.len() as u16).to_be_bytes());
        hash.update(ENCODING_LANGUAGE_V1);
        hash.finalize()
    })
}

fn source_contract_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    static DIGEST: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    *DIGEST.get_or_init(|| {
        let mut hash = Keccak256::new();
        hash.update(SOURCE_CONTRACT_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&encoding_contract_digest_v1());
        hash.update(&(OBJECTS_V1 as u16).to_be_bytes());
        hash.update(&CANONICAL_COEFFICIENTS_V1.to_be_bytes());
        hash.update(&CANONICAL_BYTES_V1.to_be_bytes());
        hash.update(&(ORDER_LANGUAGE_V1.len() as u16).to_be_bytes());
        hash.update(ORDER_LANGUAGE_V1);
        hash.update(&(SOURCE_LANGUAGE_V1.len() as u16).to_be_bytes());
        hash.update(SOURCE_LANGUAGE_V1);
        hash.update(&(PUBLICATION_RESOURCE_ACCOUNTING_SCOPE_V1.len() as u16).to_be_bytes());
        hash.update(PUBLICATION_RESOURCE_ACCOUNTING_SCOPE_V1);
        hash.finalize()
    })
}

const fn object_kind_v1(role: RnsNativePublicPolynomialRoleV1) -> ZkAmsMkheDirectObjectKindV1 {
    match role {
        RnsNativePublicPolynomialRoleV1::PublicA => ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
        RnsNativePublicPolynomialRoleV1::PublicB => ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
        RnsNativePublicPolynomialRoleV1::CiphertextC0 => {
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0
        }
        RnsNativePublicPolynomialRoleV1::CiphertextC1 => {
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1
        }
    }
}

/// One exact role/record/limb coordinate in the sole publication order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicPolynomialPositionV1 {
    ordinal: u16,
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<u8>,
    limb: u8,
    modulus: u64,
    position_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialPositionV1 {
    fn from_ordinal_v1(ordinal: usize) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        if ordinal >= OBJECTS_V1 {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition);
        }
        let b_start = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c0_start = b_start + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c1_start = c0_start + CIPHERTEXT_OBJECTS_PER_COMPONENT_V1;
        let (role, record, limb) = match ordinal {
            value if value < b_start => (RnsNativePublicPolynomialRoleV1::PublicA, None, value),
            value if value < c0_start => (
                RnsNativePublicPolynomialRoleV1::PublicB,
                None,
                value - b_start,
            ),
            value if value < c1_start => {
                let index = value - c0_start;
                (
                    RnsNativePublicPolynomialRoleV1::CiphertextC0,
                    Some(index / ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1),
                    index % ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
                )
            }
            value => {
                let index = value - c1_start;
                (
                    RnsNativePublicPolynomialRoleV1::CiphertextC1,
                    Some(index / ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1),
                    index % ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
                )
            }
        };
        let record = record
            .map(u8::try_from)
            .transpose()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?;
        let limb_u8 = u8::try_from(limb)
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?;
        let mut value = Self {
            ordinal: u16::try_from(ordinal)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            role,
            record,
            limb: limb_u8,
            modulus: ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb],
            position_digest: [0; DIGEST_BYTES_V1],
        };
        value.position_digest = position_digest_v1(value);
        value.validate_v1()?;
        Ok(value)
    }

    fn validate_v1(self) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        let ordinal = usize::from(self.ordinal);
        let canonical = Self::from_ordinal_without_validation_v1(ordinal)?;
        if self.role != canonical.role
            || self.record != canonical.record
            || self.limb != canonical.limb
            || self.modulus != canonical.modulus
            || self.position_digest == [0; DIGEST_BYTES_V1]
            || self.position_digest != position_digest_v1(self)
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition);
        }
        Ok(())
    }

    fn from_ordinal_without_validation_v1(
        ordinal: usize,
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        if ordinal >= OBJECTS_V1 {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition);
        }
        let b_start = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c0_start = b_start + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c1_start = c0_start + CIPHERTEXT_OBJECTS_PER_COMPONENT_V1;
        let (role, record, limb) = if ordinal < b_start {
            (RnsNativePublicPolynomialRoleV1::PublicA, None, ordinal)
        } else if ordinal < c0_start {
            (
                RnsNativePublicPolynomialRoleV1::PublicB,
                None,
                ordinal - b_start,
            )
        } else if ordinal < c1_start {
            let index = ordinal - c0_start;
            (
                RnsNativePublicPolynomialRoleV1::CiphertextC0,
                Some(index / ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1),
                index % ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
            )
        } else {
            let index = ordinal - c1_start;
            (
                RnsNativePublicPolynomialRoleV1::CiphertextC1,
                Some(index / ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1),
                index % ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
            )
        };
        Ok(Self {
            ordinal: u16::try_from(ordinal)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            role,
            record: record
                .map(u8::try_from)
                .transpose()
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            limb: u8::try_from(limb)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            modulus: ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb],
            position_digest: [0; DIGEST_BYTES_V1],
        })
    }

    pub(super) const fn ordinal_v1(self) -> u16 {
        self.ordinal
    }

    pub(super) const fn role_v1(self) -> RnsNativePublicPolynomialRoleV1 {
        self.role
    }

    pub(super) const fn record_v1(self) -> Option<u8> {
        self.record
    }

    pub(super) const fn limb_v1(self) -> u8 {
        self.limb
    }

    pub(super) const fn modulus_v1(self) -> u64 {
        self.modulus
    }

    pub(super) const fn position_digest_v1(self) -> [u8; DIGEST_BYTES_V1] {
        self.position_digest
    }
}

fn position_digest_v1(position: RnsNativePublicPolynomialPositionV1) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(POSITION_DOMAIN_V1);
    hash.update(&[VERSION_V1, position.role as u8]);
    hash.update(&position.ordinal.to_be_bytes());
    hash.update(&[position.record.unwrap_or(u8::MAX), position.limb]);
    hash.update(&position.modulus.to_be_bytes());
    hash.update(&encoding_contract_digest_v1());
    hash.finalize()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicPolynomialChunkRequestV1 {
    position: RnsNativePublicPolynomialPositionV1,
    chunk: u16,
    first_coefficient: u32,
    request_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialChunkRequestV1 {
    fn new_v1(
        position: RnsNativePublicPolynomialPositionV1,
        chunk: usize,
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        position.validate_v1()?;
        if chunk >= CHUNKS_PER_OBJECT_V1 {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition);
        }
        let first = chunk
            .checked_mul(COEFFICIENTS_PER_CHUNK_V1)
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        let mut value = Self {
            position,
            chunk: u16::try_from(chunk)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            first_coefficient: u32::try_from(first)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            request_digest: [0; DIGEST_BYTES_V1],
        };
        value.request_digest = chunk_request_digest_v1(value);
        Ok(value)
    }

    pub(super) const fn position_v1(self) -> RnsNativePublicPolynomialPositionV1 {
        self.position
    }

    pub(super) const fn chunk_v1(self) -> u16 {
        self.chunk
    }

    pub(super) const fn first_coefficient_v1(self) -> u32 {
        self.first_coefficient
    }

    pub(super) const fn coefficient_count_v1(self) -> u16 {
        COEFFICIENTS_PER_CHUNK_V1 as u16
    }

    pub(super) const fn request_digest_v1(self) -> [u8; DIGEST_BYTES_V1] {
        self.request_digest
    }
}

fn chunk_request_digest_v1(request: RnsNativePublicPolynomialChunkRequestV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_CONTRACT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&source_contract_digest_v1());
    hash.update(&request.position.position_digest);
    hash.update(&request.chunk.to_be_bytes());
    hash.update(&request.first_coefficient.to_be_bytes());
    hash.update(&(COEFFICIENTS_PER_CHUNK_V1 as u16).to_be_bytes());
    hash.finalize()
}

/// Move-only terminal proof issued by a sequential coefficient owner.
#[must_use = "the publication set must retain the exact source terminal"]
pub(super) struct RnsNativePublicPolynomialSourceTerminalV1 {
    source_identity: [u8; DIGEST_BYTES_V1],
    upstream_owner_digest: [u8; DIGEST_BYTES_V1],
    object_count: u16,
    coefficient_count: u64,
    terminal_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialSourceTerminalV1 {
    // Deliberately private: sibling modules may implement the streaming trait
    // to explore a future owner, but cannot mint its successful terminal. The
    // eventual governed adapter must land in this module beside the audit.
    fn new_v1(
        source_identity: [u8; DIGEST_BYTES_V1],
        object_count: u16,
        coefficient_count: u64,
        upstream_owner_digest: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        let mut value = Self {
            source_identity,
            upstream_owner_digest,
            object_count,
            coefficient_count,
            terminal_digest: [0; DIGEST_BYTES_V1],
        };
        value.terminal_digest = source_terminal_digest_v1(&value);
        value.validate_v1()?;
        Ok(value)
    }

    fn validate_v1(&self) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        if self.source_identity == [0; DIGEST_BYTES_V1]
            || self.upstream_owner_digest == [0; DIGEST_BYTES_V1]
            || self.upstream_owner_digest == self.source_identity
            || self.object_count != OBJECTS_V1 as u16
            || self.coefficient_count != CANONICAL_COEFFICIENTS_V1
            || self.terminal_digest == [0; DIGEST_BYTES_V1]
            || self.terminal_digest == self.source_identity
            || self.terminal_digest == self.upstream_owner_digest
            || self.terminal_digest != source_terminal_digest_v1(self)
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
        }
        Ok(())
    }

    pub(super) const fn terminal_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.terminal_digest
    }

    pub(super) const fn upstream_owner_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.upstream_owner_digest
    }
}

fn source_terminal_digest_v1(
    terminal: &RnsNativePublicPolynomialSourceTerminalV1,
) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_TERMINAL_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&source_contract_digest_v1());
    hash.update(&terminal.source_identity);
    hash.update(&terminal.upstream_owner_digest);
    hash.update(&terminal.object_count.to_be_bytes());
    hash.update(&terminal.coefficient_count.to_be_bytes());
    hash.finalize()
}

/// Sequential, move-only coefficient-domain source contract.
///
/// The caller consumes the source by value. Implementations must return the
/// exact next chunk requested and must not transform NTT-domain storage into a
/// claimed coefficient-domain stream implicitly. A future production adapter
/// must be constructed only by the governed 40-limb Phase-23 owner.
pub(super) trait RnsNativePublicPolynomialCoefficientSourceV1: Sized {
    fn source_identity_v1(
        &mut self,
    ) -> Result<[u8; DIGEST_BYTES_V1], RnsNativePublicPolynomialPublisherErrorV1>;

    fn fill_next_chunk_v1(
        &mut self,
        request: RnsNativePublicPolynomialChunkRequestV1,
        destination: &mut [u64; COEFFICIENTS_PER_CHUNK_V1],
    ) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1>;

    fn finish_source_v1(
        self,
    ) -> Result<RnsNativePublicPolynomialSourceTerminalV1, RnsNativePublicPolynomialPublisherErrorV1>;
}

/// No value of this type can exist. It is the sole production adapter slot.
///
/// It must be replaced with a sealed transition from an actual governed
/// 40-limb owner; adding a constructor or a variant without first closing the
/// upstream delta is a release-gate violation.
pub(super) enum RnsNativePhase23FortyLimbProductionSourceV1 {}

impl RnsNativePublicPolynomialCoefficientSourceV1 for RnsNativePhase23FortyLimbProductionSourceV1 {
    fn source_identity_v1(
        &mut self,
    ) -> Result<[u8; DIGEST_BYTES_V1], RnsNativePublicPolynomialPublisherErrorV1> {
        match *self {}
    }

    fn fill_next_chunk_v1(
        &mut self,
        _request: RnsNativePublicPolynomialChunkRequestV1,
        _destination: &mut [u64; COEFFICIENTS_PER_CHUNK_V1],
    ) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        match *self {}
    }

    fn finish_source_v1(
        self,
    ) -> Result<RnsNativePublicPolynomialSourceTerminalV1, RnsNativePublicPolynomialPublisherErrorV1>
    {
        match self {}
    }
}

fn encode_chunk_v1(
    request: RnsNativePublicPolynomialChunkRequestV1,
    coefficients: &[u64; COEFFICIENTS_PER_CHUNK_V1],
    output: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
    request.position.validate_v1()?;
    let expected_first = usize::from(request.chunk)
        .checked_mul(COEFFICIENTS_PER_CHUNK_V1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
    if usize::from(request.chunk) >= CHUNKS_PER_OBJECT_V1
        || request.first_coefficient != expected_first
        || request.request_digest != chunk_request_digest_v1(request)
        || coefficients
            .iter()
            .any(|coefficient| *coefficient >= request.position.modulus)
    {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidCoefficient);
    }
    for (encoded, coefficient) in output.chunks_exact_mut(RESIDUE_BYTES_V1).zip(coefficients) {
        encoded.copy_from_slice(&coefficient.to_be_bytes());
    }
    Ok(())
}

/// One immutable traversal plan. Production can construct only the complete
/// 131,072-coefficient plan; tests may construct a prefix-sized plan solely to
/// exercise the identical transaction chronology without allocating 3.7 GB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RnsNativePublicPolynomialTraversalPlanV1 {
    position: RnsNativePublicPolynomialPositionV1,
    coefficient_count: u32,
    chunks: u16,
    object_bytes: u64,
}

impl RnsNativePublicPolynomialTraversalPlanV1 {
    fn production_v1(
        position: RnsNativePublicPolynomialPositionV1,
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        let value = Self {
            position,
            coefficient_count: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32,
            chunks: CHUNKS_PER_OBJECT_V1 as u16,
            object_bytes: OBJECT_BYTES_V1,
        };
        value.validate_v1()?;
        Ok(value)
    }

    #[cfg(test)]
    fn fixture_v1(
        position: RnsNativePublicPolynomialPositionV1,
        chunks: usize,
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        let coefficient_count = chunks
            .checked_mul(COEFFICIENTS_PER_CHUNK_V1)
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        let object_bytes = coefficient_count
            .checked_mul(RESIDUE_BYTES_V1)
            .and_then(|bytes| bytes.checked_add(COUNT_PREFIX_BYTES_V1))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        let value = Self {
            position,
            coefficient_count: u32::try_from(coefficient_count)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?,
            chunks: u16::try_from(chunks)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition)?,
            object_bytes,
        };
        value.validate_v1()?;
        Ok(value)
    }

    fn validate_v1(self) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        self.position.validate_v1()?;
        let chunks = usize::from(self.chunks);
        let coefficient_count = usize::try_from(self.coefficient_count)
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        let expected_coefficients = chunks
            .checked_mul(COEFFICIENTS_PER_CHUNK_V1)
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        let expected_bytes = expected_coefficients
            .checked_mul(RESIDUE_BYTES_V1)
            .and_then(|bytes| bytes.checked_add(COUNT_PREFIX_BYTES_V1))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        if chunks == 0
            || chunks > CHUNKS_PER_OBJECT_V1
            || coefficient_count != expected_coefficients
            || self.object_bytes != expected_bytes
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPosition);
        }
        Ok(())
    }
}

/// The sole source-to-CAS object traversal seam.
///
/// Poisoning happens before every fallible or foreign call. If an outer panic
/// boundary catches a source/backend unwind, this value remains permanently
/// unusable; it can neither retry an object nor issue a receipt.
struct RnsNativePublicPolynomialAuthenticatedTraversalV1<'a, S, P>
where
    S: RnsNativePublicPolynomialCoefficientSourceV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    source: &'a mut S,
    publisher: &'a mut P,
    source_identity: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    coefficients: [u64; COEFFICIENTS_PER_CHUNK_V1],
    encoded: [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    poisoned: bool,
}

impl<'a, S, P> RnsNativePublicPolynomialAuthenticatedTraversalV1<'a, S, P>
where
    S: RnsNativePublicPolynomialCoefficientSourceV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    fn new_v1(
        source: &'a mut S,
        publisher: &'a mut P,
        source_identity: [u8; DIGEST_BYTES_V1],
        publication_identity: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        if source_identity == [0; DIGEST_BYTES_V1]
            || publication_identity == [0; DIGEST_BYTES_V1]
            || source_identity == publication_identity
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
        }
        Ok(Self {
            source,
            publisher,
            source_identity,
            publication_identity,
            coefficients: [u64::MAX; COEFFICIENTS_PER_CHUNK_V1],
            encoded: [0; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
            poisoned: false,
        })
    }

    fn publish_next_v1(
        &mut self,
        plan: RnsNativePublicPolynomialTraversalPlanV1,
        source_stream_hash: &mut Keccak256,
    ) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, RnsNativePublicPolynomialPublisherErrorV1>
    {
        if self.poisoned {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
        }
        self.poisoned = true;
        let result = self.publish_next_inner_v1(plan, source_stream_hash);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn publish_next_inner_v1(
        &mut self,
        plan: RnsNativePublicPolynomialTraversalPlanV1,
        source_stream_hash: &mut Keccak256,
    ) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, RnsNativePublicPolynomialPublisherErrorV1>
    {
        plan.validate_v1()?;
        if self.source.source_identity_v1()? != self.source_identity {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
        }
        source_stream_hash.update(&plan.position.position_digest);
        let mut transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
            object_kind_v1(plan.position.role),
            plan.object_bytes,
            &mut *self.publisher,
        )
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
        let count = plan.coefficient_count.to_be_bytes();
        transaction
            .write_exact(&count)
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
        source_stream_hash.update(&count);
        for chunk in 0..usize::from(plan.chunks) {
            let request = RnsNativePublicPolynomialChunkRequestV1::new_v1(plan.position, chunk)?;
            // `u64::MAX` is non-canonical for every governed modulus. A source
            // that leaves even one destination entry unwritten therefore
            // fails instead of inheriting zero or a prior chunk's residue.
            self.coefficients.fill(u64::MAX);
            if self.source.source_identity_v1()? != self.source_identity {
                return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
            }
            self.source
                .fill_next_chunk_v1(request, &mut self.coefficients)?;
            if self.source.source_identity_v1()? != self.source_identity {
                return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
            }
            encode_chunk_v1(request, &self.coefficients, &mut self.encoded)?;
            source_stream_hash.update(&request.request_digest);
            source_stream_hash.update(&self.encoded);
            transaction
                .write_exact(&self.encoded)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
        }
        if transaction.remaining_bytes() != 0 {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::Incomplete);
        }
        let receipt = transaction
            .finish()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
        validate_publication_receipt_for_bytes_v1(
            self.publication_identity,
            plan.position,
            plan.object_bytes,
            &receipt,
        )?;
        Ok(receipt)
    }

    #[cfg(test)]
    const fn is_poisoned_v1(&self) -> bool {
        self.poisoned
    }
}

struct RnsNativePublicPolynomialManifestBuilderV1 {
    next_ordinal: usize,
    public_a: Vec<RnsNativePublicPolynomialDescriptorV1>,
    public_b: Vec<RnsNativePublicPolynomialDescriptorV1>,
    ciphertext_c0: Vec<RnsNativePublicPolynomialDescriptorV1>,
    ciphertext_c1: Vec<RnsNativePublicPolynomialDescriptorV1>,
    pointer_digests: Vec<[u8; DIGEST_BYTES_V1]>,
}

impl RnsNativePublicPolynomialManifestBuilderV1 {
    fn try_new_v1() -> Result<Self, RnsNativePublicPolynomialPublisherErrorV1> {
        fn exact_vec_v1<T>(
            capacity: usize,
        ) -> Result<Vec<T>, RnsNativePublicPolynomialPublisherErrorV1> {
            let mut values = Vec::new();
            values
                .try_reserve_exact(capacity)
                .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ResourceCeilingExceeded)?;
            if values.capacity() != capacity {
                return Err(RnsNativePublicPolynomialPublisherErrorV1::ResourceCeilingExceeded);
            }
            Ok(values)
        }
        Ok(Self {
            next_ordinal: 0,
            public_a: exact_vec_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)?,
            public_b: exact_vec_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)?,
            ciphertext_c0: exact_vec_v1(CIPHERTEXT_OBJECTS_PER_COMPONENT_V1)?,
            ciphertext_c1: exact_vec_v1(CIPHERTEXT_OBJECTS_PER_COMPONENT_V1)?,
            pointer_digests: exact_vec_v1(OBJECTS_V1)?,
        })
    }

    fn absorb_pointer_v1(
        &mut self,
        position: RnsNativePublicPolynomialPositionV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        position.validate_v1()?;
        if usize::from(position.ordinal) != self.next_ordinal {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidOrder);
        }
        if pointer.kind() != object_kind_v1(position.role)
            || pointer.payload_bytes() != OBJECT_BYTES_V1
            || pointer.payload_blake3() == [0; DIGEST_BYTES_V1]
            || pointer.pointer_digest() == [0; DIGEST_BYTES_V1]
            || ZkAmsMkheDirectObjectPointerV1::decode_exact(pointer.kind(), &pointer.encode()).ok()
                != Some(pointer)
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
        }
        if self.pointer_digests.contains(&pointer.pointer_digest()) {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::DuplicatePointer);
        }
        let descriptor = RnsNativePublicPolynomialDescriptorV1::new(
            position.role,
            position.record,
            usize::from(position.limb),
            pointer,
        )
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
        match position.role {
            RnsNativePublicPolynomialRoleV1::PublicA => self.public_a.push(descriptor),
            RnsNativePublicPolynomialRoleV1::PublicB => self.public_b.push(descriptor),
            RnsNativePublicPolynomialRoleV1::CiphertextC0 => self.ciphertext_c0.push(descriptor),
            RnsNativePublicPolynomialRoleV1::CiphertextC1 => self.ciphertext_c1.push(descriptor),
        }
        self.pointer_digests.push(pointer.pointer_digest());
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(RnsNativePublicPolynomialPublisherErrorV1::ArithmeticOverflow)?;
        Ok(())
    }

    fn finish_v1(
        self,
    ) -> Result<RnsNativePublicPolynomialManifestV1, RnsNativePublicPolynomialPublisherErrorV1>
    {
        if self.next_ordinal != OBJECTS_V1
            || self.pointer_digests.len() != OBJECTS_V1
            || self.public_a.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.public_b.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.ciphertext_c0.len() != CIPHERTEXT_OBJECTS_PER_COMPONENT_V1
            || self.ciphertext_c1.len() != CIPHERTEXT_OBJECTS_PER_COMPONENT_V1
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::Incomplete);
        }
        RnsNativePublicPolynomialManifestV1::new(
            self.public_a.into_boxed_slice(),
            self.public_b.into_boxed_slice(),
            self.ciphertext_c0.into_boxed_slice(),
            self.ciphertext_c1.into_boxed_slice(),
        )
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)
    }
}

fn validate_publication_receipt_v1(
    expected_publication_identity: [u8; DIGEST_BYTES_V1],
    position: RnsNativePublicPolynomialPositionV1,
    receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
    validate_publication_receipt_for_bytes_v1(
        expected_publication_identity,
        position,
        OBJECT_BYTES_V1,
        receipt,
    )
}

fn validate_publication_receipt_for_bytes_v1(
    expected_publication_identity: [u8; DIGEST_BYTES_V1],
    position: RnsNativePublicPolynomialPositionV1,
    expected_object_bytes: u64,
    receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
    let pointer = receipt.pointer();
    let published = receipt.published_binding();
    let readback = receipt.post_publish_read_receipt();
    if expected_publication_identity == [0; DIGEST_BYTES_V1]
        || receipt.publication_identity() != expected_publication_identity
        || pointer.kind() != object_kind_v1(position.role)
        || expected_object_bytes <= COUNT_PREFIX_BYTES_V1 as u64
        || pointer.payload_bytes() != expected_object_bytes
        || pointer.payload_blake3() == [0; DIGEST_BYTES_V1]
        || pointer.pointer_digest() == [0; DIGEST_BYTES_V1]
        || published.publication_identity() != expected_publication_identity
        || published.pointer() != pointer
        || published.binding_digest() == [0; DIGEST_BYTES_V1]
        || readback.snapshot().pointer() != pointer
        || readback.canonical_bytes() != expected_object_bytes
        || readback.payload_blake3() != pointer.payload_blake3()
        || readback.receipt_digest() == [0; DIGEST_BYTES_V1]
        || receipt.receipt_digest() == [0; DIGEST_BYTES_V1]
    {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
    }
    Ok(())
}

fn publication_set_digest_v1(
    source_identity: [u8; DIGEST_BYTES_V1],
    source_owner_digest: [u8; DIGEST_BYTES_V1],
    source_stream_digest: [u8; DIGEST_BYTES_V1],
    source_terminal_digest: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    manifest_digest: [u8; DIGEST_BYTES_V1],
    receipts: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativePublicPolynomialPublisherErrorV1> {
    let identity_axes = [
        source_identity,
        source_owner_digest,
        source_stream_digest,
        source_terminal_digest,
        publication_identity,
        manifest_digest,
    ];
    if identity_axes.contains(&[0; DIGEST_BYTES_V1])
        || identity_axes
            .iter()
            .enumerate()
            .any(|(index, value)| identity_axes[index + 1..].contains(value))
        || receipts.len() != OBJECTS_V1
    {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
    }
    let mut hash = Keccak256::new();
    hash.update(PUBLICATION_SET_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&source_contract_digest_v1());
    hash.update(&source_identity);
    hash.update(&source_owner_digest);
    hash.update(&source_stream_digest);
    hash.update(&source_terminal_digest);
    hash.update(&publication_identity);
    hash.update(&manifest_digest);
    hash.update(&(OBJECTS_V1 as u16).to_be_bytes());
    hash.update(&CANONICAL_COEFFICIENTS_V1.to_be_bytes());
    hash.update(&CANONICAL_BYTES_V1.to_be_bytes());
    hash.update(&AUTHENTICATED_TRANSFER_BYTES_V1.to_be_bytes());
    hash.update(&(PUBLICATION_TRANSPORT_CALLS_V1 as u32).to_be_bytes());
    for (ordinal, receipt) in receipts.iter().enumerate() {
        let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal)?;
        validate_publication_receipt_v1(publication_identity, position, receipt)?;
        hash.update(&position.position_digest);
        hash.update(&receipt.pointer().encode());
        hash.update(&receipt.receipt_digest());
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
    }
    Ok(digest)
}

/// Move-only complete publication set, still owning the immutable provider.
#[must_use = "the only sound next step is the typed authenticated-reader handoff"]
pub(super) struct RnsNativePublicPolynomialPublishedSetV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    manifest: RnsNativePublicPolynomialManifestV1,
    provider: P,
    receipts: Box<[ZkAmsMkheDirectObjectPublicationReceiptV1]>,
    source_identity: [u8; DIGEST_BYTES_V1],
    source_owner_digest: [u8; DIGEST_BYTES_V1],
    source_stream_digest: [u8; DIGEST_BYTES_V1],
    source_terminal_digest: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    publication_set_digest: [u8; DIGEST_BYTES_V1],
}

impl<P> RnsNativePublicPolynomialPublishedSetV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    fn validate_v1(&self) -> Result<(), RnsNativePublicPolynomialPublisherErrorV1> {
        let manifest_digest = self.manifest.manifest_digest_v1();
        let expected = publication_set_digest_v1(
            self.source_identity,
            self.source_owner_digest,
            self.source_stream_digest,
            self.source_terminal_digest,
            self.publication_identity,
            manifest_digest,
            &self.receipts,
        )?;
        if self.publication_set_digest == [0; DIGEST_BYTES_V1]
            || self.publication_set_digest != expected
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
        }
        for (ordinal, receipt) in self.receipts.iter().enumerate() {
            let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal)?;
            let artifact = self
                .manifest
                .statement_artifact_digest_v1(
                    position.role,
                    position.record.map(usize::from),
                    usize::from(position.limb),
                )
                .ok_or(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
            let reconstructed = RnsNativePublicPolynomialDescriptorV1::new(
                position.role,
                position.record,
                usize::from(position.limb),
                receipt.pointer(),
            )
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
            if artifact != reconstructed.artifact_digest_v1() {
                return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
            }
        }
        Ok(())
    }

    pub(super) const fn publication_set_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.publication_set_digest
    }

    pub(super) fn into_reader_handoff_v1(
        self,
    ) -> Result<
        RnsNativePublicPolynomialReaderHandoffV1<P>,
        RnsNativePublicPolynomialPublisherErrorV1,
    > {
        self.validate_v1()?;
        let manifest_digest = self.manifest.manifest_digest_v1();
        let reader = RnsNativePublicPolynomialReaderV1::new(self.manifest, self.provider)
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?;
        let handoff_digest = reader_handoff_digest_v1(
            manifest_digest,
            self.source_identity,
            self.source_owner_digest,
            self.source_stream_digest,
            self.source_terminal_digest,
            self.publication_identity,
            self.publication_set_digest,
        );
        if handoff_digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        Ok(RnsNativePublicPolynomialReaderHandoffV1 {
            reader,
            receipts: self.receipts,
            manifest_digest,
            source_identity: self.source_identity,
            source_owner_digest: self.source_owner_digest,
            source_stream_digest: self.source_stream_digest,
            source_terminal_digest: self.source_terminal_digest,
            publication_identity: self.publication_identity,
            publication_set_digest: self.publication_set_digest,
            handoff_digest,
        })
    }
}

fn reader_handoff_digest_v1(
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_identity: [u8; DIGEST_BYTES_V1],
    source_owner_digest: [u8; DIGEST_BYTES_V1],
    source_stream_digest: [u8; DIGEST_BYTES_V1],
    source_terminal_digest: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    publication_set_digest: [u8; DIGEST_BYTES_V1],
) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(READER_HANDOFF_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&manifest_digest);
    hash.update(&source_identity);
    hash.update(&source_owner_digest);
    hash.update(&source_stream_digest);
    hash.update(&source_terminal_digest);
    hash.update(&publication_identity);
    hash.update(&publication_set_digest);
    hash.update(&(OBJECTS_V1 as u16).to_be_bytes());
    hash.finalize()
}

/// Reader plus the publication evidence it is forbidden to detach from.
#[must_use = "finish the complete authenticated read to consume this handoff"]
pub(super) struct RnsNativePublicPolynomialReaderHandoffV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    reader: RnsNativePublicPolynomialReaderV1<P>,
    receipts: Box<[ZkAmsMkheDirectObjectPublicationReceiptV1]>,
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_identity: [u8; DIGEST_BYTES_V1],
    source_owner_digest: [u8; DIGEST_BYTES_V1],
    source_stream_digest: [u8; DIGEST_BYTES_V1],
    source_terminal_digest: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    publication_set_digest: [u8; DIGEST_BYTES_V1],
    handoff_digest: [u8; DIGEST_BYTES_V1],
}

impl<P> RnsNativePublicPolynomialReaderHandoffV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    pub(super) fn take_next_evaluation_v1(
        &mut self,
        schedule: &RnsNativeQpcsRelationScheduleV1,
        limb: usize,
        repetition: usize,
    ) -> Result<RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReaderErrorV1> {
        self.reader
            .take_next_evaluation_v1(schedule, limb, repetition)
    }

    pub(super) const fn handoff_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.handoff_digest
    }

    pub(super) fn finish_v1(
        self,
    ) -> Result<
        RnsNativePublicPolynomialPublishedReadReceiptV1,
        RnsNativePublicPolynomialPublisherErrorV1,
    > {
        if self.receipts.len() != OBJECTS_V1
            || self.handoff_digest
                != reader_handoff_digest_v1(
                    self.manifest_digest,
                    self.source_identity,
                    self.source_owner_digest,
                    self.source_stream_digest,
                    self.source_terminal_digest,
                    self.publication_identity,
                    self.publication_set_digest,
                )
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        let read_receipt = self
            .reader
            .finish()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff)?;
        if read_receipt.manifest_digest_v1() != self.manifest_digest
            || read_receipt.object_count_v1() != OBJECTS_V1 as u16
            || read_receipt.canonical_bytes_v1() != CANONICAL_BYTES_V1
            || read_receipt.coefficient_count_v1() != CANONICAL_COEFFICIENTS_V1
        {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        let mut hash = Keccak256::new();
        hash.update(PUBLISHED_READ_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&self.handoff_digest);
        hash.update(&self.publication_set_digest);
        hash.update(&read_receipt.read_set_digest_v1());
        hash.update(&read_receipt.qpcs_schedule_digest_v1());
        let digest = hash.finalize();
        if digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativePublicPolynomialPublisherErrorV1::ReaderHandoff);
        }
        Ok(RnsNativePublicPolynomialPublishedReadReceiptV1 {
            read_receipt,
            source_identity: self.source_identity,
            source_owner_digest: self.source_owner_digest,
            source_terminal_digest: self.source_terminal_digest,
            publication_identity: self.publication_identity,
            publication_set_digest: self.publication_set_digest,
            handoff_digest: self.handoff_digest,
            digest,
        })
    }
}

/// Move-only receipt binding the complete publication and complete reader pass.
#[must_use = "the future direct-source transition must consume this receipt"]
pub(super) struct RnsNativePublicPolynomialPublishedReadReceiptV1 {
    read_receipt: RnsNativePublicPolynomialReadReceiptV1,
    source_identity: [u8; DIGEST_BYTES_V1],
    source_owner_digest: [u8; DIGEST_BYTES_V1],
    source_terminal_digest: [u8; DIGEST_BYTES_V1],
    publication_identity: [u8; DIGEST_BYTES_V1],
    publication_set_digest: [u8; DIGEST_BYTES_V1],
    handoff_digest: [u8; DIGEST_BYTES_V1],
    digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialPublishedReadReceiptV1 {
    pub(super) const fn read_receipt_v1(&self) -> &RnsNativePublicPolynomialReadReceiptV1 {
        &self.read_receipt
    }

    pub(super) const fn source_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.source_identity
    }

    pub(super) const fn source_owner_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.source_owner_digest
    }

    pub(super) const fn source_terminal_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.source_terminal_digest
    }

    pub(super) const fn publication_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.publication_identity
    }

    pub(super) const fn publication_set_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.publication_set_digest
    }

    pub(super) const fn handoff_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.handoff_digest
    }

    pub(super) const fn digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.digest
    }
}

/// Consume an exact source and CAS publisher and publish all 3,520 objects.
///
/// Every object uses the existing transaction's mutable-stage write,
/// immutable-seal reread, authoritative pointer publication/reconciliation,
/// and complete post-publication provider readback. Any error returns no
/// manifest, reader, source terminal, or admission capability.
pub(super) fn publish_rns_native_public_polynomials_v1<S, P>(
    mut source: S,
    mut publisher: P,
) -> Result<RnsNativePublicPolynomialPublishedSetV1<P>, RnsNativePublicPolynomialPublisherErrorV1>
where
    S: RnsNativePublicPolynomialCoefficientSourceV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1,
{
    let source_identity = source.source_identity_v1()?;
    if source_identity == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
    }
    let publication_identity = publisher
        .publication_identity()
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?;
    if publication_identity == [0; DIGEST_BYTES_V1] || publication_identity == source_identity {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication);
    }
    let mut receipts = Vec::new();
    receipts
        .try_reserve_exact(OBJECTS_V1)
        .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::ResourceCeilingExceeded)?;
    if receipts.capacity() != OBJECTS_V1 {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::ResourceCeilingExceeded);
    }
    let mut manifest = RnsNativePublicPolynomialManifestBuilderV1::try_new_v1()?;
    let mut source_stream_hash = Keccak256::new();
    source_stream_hash.update(SOURCE_STREAM_DOMAIN_V1);
    source_stream_hash.update(&[VERSION_V1]);
    source_stream_hash.update(&source_contract_digest_v1());
    source_stream_hash.update(&source_identity);
    source_stream_hash.update(&(OBJECTS_V1 as u16).to_be_bytes());
    {
        let mut traversal = RnsNativePublicPolynomialAuthenticatedTraversalV1::new_v1(
            &mut source,
            &mut publisher,
            source_identity,
            publication_identity,
        )?;
        for ordinal in 0..OBJECTS_V1 {
            let position = RnsNativePublicPolynomialPositionV1::from_ordinal_v1(ordinal)?;
            let plan = RnsNativePublicPolynomialTraversalPlanV1::production_v1(position)?;
            let receipt = traversal.publish_next_v1(plan, &mut source_stream_hash)?;
            validate_publication_receipt_v1(publication_identity, position, &receipt)?;
            manifest.absorb_pointer_v1(position, receipt.pointer())?;
            receipts.push(receipt);
        }
    }
    if source.source_identity_v1()? != source_identity
        || publisher
            .publication_identity()
            .map_err(|_| RnsNativePublicPolynomialPublisherErrorV1::InvalidPublication)?
            != publication_identity
        || receipts.len() != OBJECTS_V1
    {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
    }
    let terminal = source.finish_source_v1()?;
    terminal.validate_v1()?;
    if terminal.source_identity != source_identity {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
    }
    let source_stream_digest = source_stream_hash.finalize();
    if source_stream_digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativePublicPolynomialPublisherErrorV1::InvalidSource);
    }
    let manifest = manifest.finish_v1()?;
    let manifest_digest = manifest.manifest_digest_v1();
    let publication_set_digest = publication_set_digest_v1(
        source_identity,
        terminal.upstream_owner_digest,
        source_stream_digest,
        terminal.terminal_digest,
        publication_identity,
        manifest_digest,
        &receipts,
    )?;
    let published = RnsNativePublicPolynomialPublishedSetV1 {
        manifest,
        provider: publisher,
        receipts: receipts.into_boxed_slice(),
        source_identity,
        source_owner_digest: terminal.upstream_owner_digest,
        source_stream_digest,
        source_terminal_digest: terminal.terminal_digest,
        publication_identity,
        publication_set_digest,
    };
    published.validate_v1()?;
    Ok(published)
}

#[cfg(test)]
#[path = "rns_native_public_polynomial_publisher_tests.rs"]
mod tests;
