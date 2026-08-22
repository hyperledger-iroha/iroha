//! Bounded authenticated reader for the RNS-native public RLWE polynomials.
//!
//! This private module settles the first source tranche:
//! the exact pointer manifest, coefficient encoding, authenticated traversal,
//! and five-point evaluation algorithm. Declaring it does not by itself provide
//! the missing 40-limb publisher, source-preflight owner transition, sealed
//! direct numeric source, aggregate resource evidence, or release authority.
//!
//! A manifest contains exactly `A[40]`, `B[40]`, `C0[43][40]`, and
//! `C1[43][40]`. Each limb object has the sole encoding
//! `u32(131072)-big-endian || 131072*u64-big-endian`, in ascending coefficient
//! order and in the coefficient domain. Every encoded residue must be strictly
//! less than its position's corrected 40-limb modulus; reduction of malformed
//! input, an NTT-domain interpretation, and a legacy 38-limb truncation are all
//! forbidden.
//!
//! The reader owns its provider, captures one provider/snapshot pair, and is
//! move-only and poison-on-failure. It uses the existing complete direct-object
//! read transaction, so no evaluation leaves the reader before all bytes of its
//! object have matched the pointer's BLAKE3 content address. Before returning
//! any repetition for a limb, all 88 public objects needed by that limb have
//! authenticated. The sole live numeric cache is therefore five evaluations of
//! `A,B,C0[43],C1[43]`, exactly 3,520 bytes.

#![allow(
    dead_code,
    reason = "the private reader remains fail-closed until the remaining production integration delta below is implemented"
)]

use std::sync::OnceLock;

use super::{
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1,
    },
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1,
    },
    rns_native_qpcs_prefix::RnsNativeQpcsRelationScheduleV1,
};
use crate::vega::sponge::Keccak256;

const VERSION_V1: u8 = 1;
const DIGEST_BYTES_V1: usize = 32;
const RECORDS_V1: usize = 43;
const REPETITIONS_V1: usize = 5;
const PUBLIC_KEY_POLYNOMIALS_V1: usize = 2;
const CIPHERTEXT_COMPONENTS_V1: usize = 2;
const POLYNOMIALS_PER_LIMB_V1: usize =
    PUBLIC_KEY_POLYNOMIALS_V1 + CIPHERTEXT_COMPONENTS_V1 * RECORDS_V1;
const PUBLIC_POLYNOMIAL_OBJECTS_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * POLYNOMIALS_PER_LIMB_V1;
const PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1: usize = RECORDS_V1 * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
const LEGACY_RNS_LIMBS_V1: usize = 38;
const LIMB_COUNT_PREFIX_BYTES_V1: usize = 4;
const RESIDUE_BYTES_V1: usize = core::mem::size_of::<u64>();
const COEFFICIENTS_PER_READ_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / RESIDUE_BYTES_V1;
const READS_PER_LIMB_OBJECT_V1: usize =
    1 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / COEFFICIENTS_PER_READ_V1;
const COEFFICIENT_READS_PER_OBJECT_V1: usize = READS_PER_LIMB_OBJECT_V1 - 1;
const LIMB_OBJECT_BYTES_V1: u64 =
    (LIMB_COUNT_PREFIX_BYTES_V1 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1) as u64;
const PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1: u64 =
    PUBLIC_POLYNOMIAL_OBJECTS_V1 as u64 * LIMB_OBJECT_BYTES_V1;
const PUBLIC_POLYNOMIAL_POINTER_FRAME_BYTES_V1: usize =
    PUBLIC_POLYNOMIAL_OBJECTS_V1 * ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1;
const PUBLIC_POLYNOMIAL_READ_CALLS_V1: usize =
    PUBLIC_POLYNOMIAL_OBJECTS_V1 * READS_PER_LIMB_OBJECT_V1;
const PUBLIC_POLYNOMIAL_COEFFICIENTS_V1: u64 =
    PUBLIC_POLYNOMIAL_OBJECTS_V1 as u64 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64;
const MODULAR_MULTIPLICATIONS_PER_OBJECT_V1: u64 = REPETITIONS_V1 as u64
    * (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
        + COEFFICIENT_READS_PER_OBJECT_V1 as u64
        + (COEFFICIENT_READS_PER_OBJECT_V1 - 1) as u64);
const MODULAR_ADDITIONS_PER_OBJECT_V1: u64 = REPETITIONS_V1 as u64
    * (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64 + COEFFICIENT_READS_PER_OBJECT_V1 as u64);
// Binary exponentiation of `a^1024` performs eleven squarings and one selected
// multiplication under the fixed implementation below.
const BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1: u64 = 12;
const PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1: u64 = PUBLIC_POLYNOMIAL_OBJECTS_V1 as u64
    * MODULAR_MULTIPLICATIONS_PER_OBJECT_V1
    + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u64
        * REPETITIONS_V1 as u64
        * BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1;
const PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1: u64 =
    PUBLIC_POLYNOMIAL_OBJECTS_V1 as u64 * MODULAR_ADDITIONS_PER_OBJECT_V1;
// One unit per authenticated byte, canonicality check, modular multiplication,
// and modular addition. This is the exact coarse tranche accounting, not the
// still-missing aggregate measured release evidence.
const PUBLIC_POLYNOMIAL_COARSE_WORK_UNITS_V1: u64 = PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1
    + PUBLIC_POLYNOMIAL_COEFFICIENTS_V1
    + PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1
    + PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1;

const ENCODING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.encoding";
const ARTIFACT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.artifact";
const MANIFEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.manifest";
const READ_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.read-set";
const QPCS_SCHEDULE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-public-polynomial.qpcs-schedule";
const ENCODING_LANGUAGE_V1: &[u8] = b"coefficient-domain;ascending-c0-through-c131071;u32-count-big-endian-then-count-u64-big-endian;count=131072;strict-residue-less-than-position-modulus;no-reduction;no-ntt-order;one-complete-blake3-transaction-before-evaluation-escape";
const MANIFEST_LANGUAGE_V1: &[u8] = b"A[40]-then-B[40]-then-C0[record-major-43][limb-major-40]-then-C1[record-major-43][limb-major-40];descriptor-role-record-limb-modulus-kind-length-and-pointer-frame-bound;all-3520-pointer-digests-distinct;legacy-38-and-missing-new-limbs-rejected";
const READ_LANGUAGE_V1: &[u8] = b"capture-one-provider-and-snapshot;limb-major-runtime-read-A-B-then-record-major-C0-C1;five-qpcs-points-per-limb-from-one-retained-schedule;bind-all-200-points-in-read-receipt;authenticate-all-88-objects-before-serving-five-repetitions;strict-limb-then-repetition-order;poison-on-any-failure;8192-byte-maximum-read;3520-byte-evaluation-cache";

/// Exact remaining changes required before this private tranche can enter a
/// production path. No item in this file performs those changes.
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_REMAINING_INTEGRATION_DELTA_V1: &[u8] = b"construct-manifest-from-a-40-limb-phase23-publication-owner-and-move-its-immutable-provider;replace-the-detached-RnsNativePublicArtifactViewV1-preflight-input-with-this-owned-reader;derive-every-source-statement-limb-identity-from-descriptor-artifact_digest_v1;thread-provider-P-through-RnsNativeRlweSourceStatementStageV1-and-RnsNativeSourceTerminalCrossFieldPrerequisiteV1;bind-qpcs_schedule_digest-and-read_set_digest-into-the-source-terminal-token;consume-the-retained-qpcs-schedule-and-authenticated-evaluation-bytes-through-a-purpose-specific-sealed-direct-source-transition;remove-production-caller-supplied-a-A-B-C0-C1-qpcs-numeric-values;charge-aggregate-io-work-and-measured-rss;keep-composite-readiness-and-release-false-until-upstream-40-limb-KATs-and-resource-evidence-pass";

pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_SOURCE_SETTLED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_DECLARED_V1: bool = true;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_MANIFEST_INTEGRATED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_SOURCE_PREFLIGHT_INTEGRATED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_DIRECT_NUMERIC_SOURCE_INTEGRATED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_QUALIFIED_V1: bool = false;
pub(super) const RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_READY_V1: bool = false;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(LEGACY_RNS_LIMBS_V1 == 38);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(RECORDS_V1 == 43);
    assert!(REPETITIONS_V1 == 5);
    assert!(POLYNOMIALS_PER_LIMB_V1 == 88);
    assert!(PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1 == 1_720);
    assert!(PUBLIC_POLYNOMIAL_OBJECTS_V1 == 3_520);
    assert!(LIMB_OBJECT_BYTES_V1 == 1_048_580);
    assert!(COEFFICIENTS_PER_READ_V1 == 1_024);
    assert!(COEFFICIENT_READS_PER_OBJECT_V1 == 128);
    assert!(READS_PER_LIMB_OBJECT_V1 == 129);
    assert!(PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1 == 3_691_001_600);
    assert!(PUBLIC_POLYNOMIAL_POINTER_FRAME_BYTES_V1 == 274_560);
    assert!(PUBLIC_POLYNOMIAL_READ_CALLS_V1 == 454_080);
    assert!(PUBLIC_POLYNOMIAL_COEFFICIENTS_V1 == 461_373_440);
    assert!(MODULAR_MULTIPLICATIONS_PER_OBJECT_V1 == 656_635);
    assert!(MODULAR_ADDITIONS_PER_OBJECT_V1 == 656_000);
    assert!(PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1 == 2_311_357_600);
    assert!(PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1 == 2_309_120_000);
    assert!(PUBLIC_POLYNOMIAL_COARSE_WORK_UNITS_V1 == 8_772_852_640);
    assert!(PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1 < ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1);
    assert!(PUBLIC_POLYNOMIAL_COARSE_WORK_UNITS_V1 < ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 == 8_192);
    assert!(ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1 == 78);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_SOURCE_SETTLED_V1);
    assert!(RNS_NATIVE_PUBLIC_POLYNOMIAL_READER_DECLARED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_UPSTREAM_40_LIMB_MANIFEST_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_SOURCE_PREFLIGHT_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_DIRECT_NUMERIC_SOURCE_INTEGRATED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_MEASURED_RSS_QUALIFIED_V1);
    assert!(!RNS_NATIVE_PUBLIC_POLYNOMIAL_PRODUCTION_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativePublicPolynomialReaderErrorV1 {
    InvalidCount,
    InvalidOrder,
    InvalidRole,
    InvalidPointer,
    DuplicatePointer,
    InvalidSchedule,
    NonCanonicalCoefficient,
    Authentication,
    SourceUnavailable,
    ResourceCeilingExceeded,
    ArithmeticOverflow,
    Poisoned,
    Incomplete,
}

impl core::fmt::Display for RnsNativePublicPolynomialReaderErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativePublicPolynomialReaderErrorV1 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum RnsNativePublicPolynomialRoleV1 {
    PublicA = 0,
    PublicB = 1,
    CiphertextC0 = 2,
    CiphertextC1 = 3,
}

impl RnsNativePublicPolynomialRoleV1 {
    const fn object_kind_v1(self) -> ZkAmsMkheDirectObjectKindV1 {
        match self {
            Self::PublicA => ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            Self::PublicB => ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            Self::CiphertextC0 => ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            Self::CiphertextC1 => ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
        }
    }

    const fn requires_record_v1(self) -> bool {
        matches!(self, Self::CiphertextC0 | Self::CiphertextC1)
    }
}

fn public_polynomial_encoding_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    static DIGEST: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    *DIGEST.get_or_init(|| {
        let mut hash = Keccak256::new();
        hash.update(ENCODING_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
        hash.update(&(LIMB_COUNT_PREFIX_BYTES_V1 as u16).to_be_bytes());
        hash.update(&(RESIDUE_BYTES_V1 as u16).to_be_bytes());
        hash.update(&LIMB_OBJECT_BYTES_V1.to_be_bytes());
        hash.update(&(ENCODING_LANGUAGE_V1.len() as u16).to_be_bytes());
        hash.update(ENCODING_LANGUAGE_V1);
        hash.finalize()
    })
}

/// One role- and position-bound content address in the exact public manifest.
///
/// This descriptor is copyable for bounded internal traversal; the manifest and
/// reader owners deliberately are not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicPolynomialDescriptorV1 {
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<u8>,
    limb: u8,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    artifact_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialDescriptorV1 {
    pub(super) fn new(
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<u8>,
        limb: usize,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Self, RnsNativePublicPolynomialReaderErrorV1> {
        if limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || role.requires_record_v1() != record.is_some()
            || record.is_some_and(|record| usize::from(record) >= RECORDS_V1)
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder);
        }
        validate_pointer_v1(role, pointer)?;
        let mut descriptor = Self {
            role,
            record,
            limb: u8::try_from(limb)
                .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)?,
            pointer,
            artifact_digest: [0; DIGEST_BYTES_V1],
        };
        descriptor.artifact_digest = artifact_digest_v1(descriptor);
        descriptor.validate_v1()?;
        Ok(descriptor)
    }

    fn validate_v1(self) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        validate_pointer_v1(self.role, self.pointer)?;
        let limb = usize::from(self.limb);
        if limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.role.requires_record_v1() != self.record.is_some()
            || self
                .record
                .is_some_and(|record| usize::from(record) >= RECORDS_V1)
            || self.artifact_digest == [0; DIGEST_BYTES_V1]
            || self.artifact_digest != artifact_digest_v1(self)
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidPointer);
        }
        Ok(())
    }

    fn validate_position_v1(
        self,
        expected_role: RnsNativePublicPolynomialRoleV1,
        expected_record: Option<usize>,
        expected_limb: usize,
    ) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        self.validate_v1()?;
        if self.role != expected_role {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidRole);
        }
        if self.record.map(usize::from) != expected_record
            || usize::from(self.limb) != expected_limb
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder);
        }
        Ok(())
    }

    pub(super) const fn artifact_digest_v1(self) -> [u8; DIGEST_BYTES_V1] {
        self.artifact_digest
    }

    pub(super) const fn pointer_v1(self) -> ZkAmsMkheDirectObjectPointerV1 {
        self.pointer
    }
}

fn validate_pointer_v1(
    role: RnsNativePublicPolynomialRoleV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
    let expected_kind = role.object_kind_v1();
    if pointer.kind() != expected_kind {
        return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidRole);
    }
    if pointer.payload_bytes() != LIMB_OBJECT_BYTES_V1
        || pointer.payload_blake3() == [0; DIGEST_BYTES_V1]
        || pointer.pointer_digest() == [0; DIGEST_BYTES_V1]
        || ZkAmsMkheDirectObjectPointerV1::decode_exact(expected_kind, &pointer.encode()).ok()
            != Some(pointer)
    {
        return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidPointer);
    }
    Ok(())
}

fn artifact_digest_v1(descriptor: RnsNativePublicPolynomialDescriptorV1) -> [u8; DIGEST_BYTES_V1] {
    let limb = usize::from(descriptor.limb);
    let mut hash = Keccak256::new();
    hash.update(ARTIFACT_DOMAIN_V1);
    hash.update(&[VERSION_V1, descriptor.role as u8]);
    hash.update(&[descriptor.record.unwrap_or(u8::MAX), descriptor.limb]);
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes());
    hash.update(&public_polynomial_encoding_digest_v1());
    hash.update(&descriptor.pointer.encode());
    hash.finalize()
}

/// Exact move-only compact manifest for all 3,520 public limb objects.
#[must_use = "dropping the manifest discards the only typed public-artifact inventory"]
pub(super) struct RnsNativePublicPolynomialManifestV1 {
    public_a: Box<[RnsNativePublicPolynomialDescriptorV1]>,
    public_b: Box<[RnsNativePublicPolynomialDescriptorV1]>,
    ciphertext_c0: Box<[RnsNativePublicPolynomialDescriptorV1]>,
    ciphertext_c1: Box<[RnsNativePublicPolynomialDescriptorV1]>,
    manifest_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialManifestV1 {
    pub(super) fn new(
        public_a: Box<[RnsNativePublicPolynomialDescriptorV1]>,
        public_b: Box<[RnsNativePublicPolynomialDescriptorV1]>,
        ciphertext_c0: Box<[RnsNativePublicPolynomialDescriptorV1]>,
        ciphertext_c1: Box<[RnsNativePublicPolynomialDescriptorV1]>,
    ) -> Result<Self, RnsNativePublicPolynomialReaderErrorV1> {
        let mut manifest = Self {
            public_a,
            public_b,
            ciphertext_c0,
            ciphertext_c1,
            manifest_digest: [0; DIGEST_BYTES_V1],
        };
        manifest.validate_shape_and_entries_v1()?;
        manifest.manifest_digest = manifest_digest_v1(&manifest)?;
        manifest.validate_v1()?;
        Ok(manifest)
    }

    fn validate_shape_and_entries_v1(&self) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        if self.public_a.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.public_b.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.ciphertext_c0.len() != PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1
            || self.ciphertext_c1.len() != PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount);
        }
        for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            self.public_a[limb].validate_position_v1(
                RnsNativePublicPolynomialRoleV1::PublicA,
                None,
                limb,
            )?;
            self.public_b[limb].validate_position_v1(
                RnsNativePublicPolynomialRoleV1::PublicB,
                None,
                limb,
            )?;
        }
        for record in 0..RECORDS_V1 {
            for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
                let index = record * ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 + limb;
                self.ciphertext_c0[index].validate_position_v1(
                    RnsNativePublicPolynomialRoleV1::CiphertextC0,
                    Some(record),
                    limb,
                )?;
                self.ciphertext_c1[index].validate_position_v1(
                    RnsNativePublicPolynomialRoleV1::CiphertextC1,
                    Some(record),
                    limb,
                )?;
            }
        }
        for ordinal in 0..PUBLIC_POLYNOMIAL_OBJECTS_V1 {
            let descriptor = self
                .descriptor_by_manifest_ordinal_v1(ordinal)
                .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)?;
            for prior in 0..ordinal {
                let prior = self
                    .descriptor_by_manifest_ordinal_v1(prior)
                    .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)?;
                if descriptor.pointer.pointer_digest() == prior.pointer.pointer_digest() {
                    return Err(RnsNativePublicPolynomialReaderErrorV1::DuplicatePointer);
                }
            }
        }
        Ok(())
    }

    fn validate_v1(&self) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        self.validate_shape_and_entries_v1()?;
        if self.manifest_digest == [0; DIGEST_BYTES_V1]
            || self.manifest_digest != manifest_digest_v1(self)?
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidPointer);
        }
        Ok(())
    }

    fn descriptor_by_manifest_ordinal_v1(
        &self,
        ordinal: usize,
    ) -> Option<RnsNativePublicPolynomialDescriptorV1> {
        let b_start = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c0_start = b_start + ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
        let c1_start = c0_start + PUBLIC_CIPHERTEXT_LIMB_OBJECTS_V1;
        match ordinal {
            value if value < b_start => self.public_a.get(value).copied(),
            value if value < c0_start => self.public_b.get(value - b_start).copied(),
            value if value < c1_start => self.ciphertext_c0.get(value - c0_start).copied(),
            value if value < PUBLIC_POLYNOMIAL_OBJECTS_V1 => {
                self.ciphertext_c1.get(value - c1_start).copied()
            }
            _ => None,
        }
    }

    fn descriptor_for_runtime_read_v1(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Option<RnsNativePublicPolynomialDescriptorV1> {
        if limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
            return None;
        }
        match role {
            RnsNativePublicPolynomialRoleV1::PublicA if record.is_none() => {
                self.public_a.get(limb).copied()
            }
            RnsNativePublicPolynomialRoleV1::PublicB if record.is_none() => {
                self.public_b.get(limb).copied()
            }
            RnsNativePublicPolynomialRoleV1::CiphertextC0 => {
                let record = record?;
                let index = record
                    .checked_mul(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)?
                    .checked_add(limb)?;
                self.ciphertext_c0.get(index).copied()
            }
            RnsNativePublicPolynomialRoleV1::CiphertextC1 => {
                let record = record?;
                let index = record
                    .checked_mul(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)?
                    .checked_add(limb)?;
                self.ciphertext_c1.get(index).copied()
            }
            _ => None,
        }
    }

    pub(super) const fn manifest_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.manifest_digest
    }

    pub(super) fn statement_artifact_digest_v1(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Option<[u8; DIGEST_BYTES_V1]> {
        self.descriptor_for_runtime_read_v1(role, record, limb)
            .map(RnsNativePublicPolynomialDescriptorV1::artifact_digest_v1)
    }
}

fn manifest_digest_v1(
    manifest: &RnsNativePublicPolynomialManifestV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativePublicPolynomialReaderErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&public_polynomial_encoding_digest_v1());
    hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
    hash.update(&(RECORDS_V1 as u16).to_be_bytes());
    hash.update(&(PUBLIC_POLYNOMIAL_OBJECTS_V1 as u16).to_be_bytes());
    hash.update(&LIMB_OBJECT_BYTES_V1.to_be_bytes());
    for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
        hash.update(&(limb as u16).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
    }
    hash.update(&(MANIFEST_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(MANIFEST_LANGUAGE_V1);
    for ordinal in 0..PUBLIC_POLYNOMIAL_OBJECTS_V1 {
        let descriptor = manifest
            .descriptor_by_manifest_ordinal_v1(ordinal)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidCount)?;
        hash.update(&(ordinal as u16).to_be_bytes());
        hash.update(&descriptor.artifact_digest);
    }
    Ok(hash.finalize())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QpcsScheduleIdentityV1 {
    parameter_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

impl QpcsScheduleIdentityV1 {
    fn from_schedule_v1(
        schedule: &RnsNativeQpcsRelationScheduleV1,
    ) -> Result<Self, RnsNativePublicPolynomialReaderErrorV1> {
        let mut value = Self {
            parameter_digest: schedule.parameter_digest(),
            q_mask_s_root: schedule.q_mask_s_root(),
            pre_relation_transcript_digest: schedule.qpcs_pre_relation_transcript_digest(),
            relation_seed: schedule.relation_seed(),
            binding_digest: [0; DIGEST_BYTES_V1],
        };
        value.validate_base_v1()?;
        let mut hash = Keccak256::new();
        hash.update(QPCS_SCHEDULE_DOMAIN_V1);
        hash.update(&[VERSION_V1]);
        hash.update(&value.parameter_digest);
        hash.update(&value.q_mask_s_root);
        hash.update(&value.pre_relation_transcript_digest);
        hash.update(&value.relation_seed);
        hash.update(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
        hash.update(&(REPETITIONS_V1 as u16).to_be_bytes());
        for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
            hash.update(&(limb as u16).to_be_bytes());
            hash.update(&modulus.to_be_bytes());
            let mut limb_points = [0_u64; REPETITIONS_V1];
            for (repetition, point) in limb_points.iter_mut().enumerate() {
                *point = schedule
                    .point(limb, repetition)
                    .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)?;
                if *point == 0 || *point >= modulus {
                    return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
                }
                hash.update(&(repetition as u16).to_be_bytes());
                hash.update(&point.to_be_bytes());
            }
            if limb_points
                .iter()
                .enumerate()
                .any(|(index, point)| limb_points[index + 1..].contains(point))
            {
                return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
            }
        }
        value.binding_digest = hash.finalize();
        value.validate_v1()?;
        Ok(value)
    }

    fn validate_base_v1(self) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        let values = [
            self.parameter_digest,
            self.q_mask_s_root,
            self.pre_relation_transcript_digest,
            self.relation_seed,
        ];
        if values.contains(&[0; DIGEST_BYTES_V1])
            || values
                .iter()
                .enumerate()
                .any(|(index, value)| values[index + 1..].contains(value))
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
        }
        Ok(())
    }

    fn validate_v1(self) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        self.validate_base_v1()?;
        if self.binding_digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct FivePointBlockPlanV1 {
    modulus: u64,
    points: [u64; REPETITIONS_V1],
    block_steps: [u64; REPETITIONS_V1],
    ring_degree: usize,
    coefficients_per_block: usize,
    block_count: usize,
    step_multiplications: u64,
}

impl FivePointBlockPlanV1 {
    fn new_v1(
        modulus: u64,
        points: [u64; REPETITIONS_V1],
        ring_degree: usize,
        coefficients_per_block: usize,
    ) -> Result<Self, RnsNativePublicPolynomialReaderErrorV1> {
        if modulus < 3
            || ring_degree == 0
            || coefficients_per_block == 0
            || coefficients_per_block
                .checked_mul(RESIDUE_BYTES_V1)
                .is_none()
            || !ring_degree.is_multiple_of(coefficients_per_block)
            || points.iter().any(|point| *point == 0 || *point >= modulus)
            || points
                .iter()
                .enumerate()
                .any(|(index, point)| points[index + 1..].contains(point))
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
        }
        let mut block_steps = [0_u64; REPETITIONS_V1];
        let mut step_multiplications = 0_u64;
        let block_exponent = u64::try_from(coefficients_per_block)
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        for (destination, point) in block_steps.iter_mut().zip(points) {
            let (step, work) = mod_pow_with_work_v1(point, block_exponent, modulus);
            *destination = step;
            step_multiplications = step_multiplications
                .checked_add(work)
                .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        }
        Ok(Self {
            modulus,
            points,
            block_steps,
            ring_degree,
            coefficients_per_block,
            block_count: ring_degree / coefficients_per_block,
            step_multiplications,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct EvaluationWorkV1 {
    coefficients: u64,
    multiplications: u64,
    additions: u64,
}

impl EvaluationWorkV1 {
    fn charge_v1(
        &mut self,
        coefficients: u64,
        multiplications: u64,
        additions: u64,
    ) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        self.coefficients = self
            .coefficients
            .checked_add(coefficients)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        self.multiplications = self
            .multiplications
            .checked_add(multiplications)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        self.additions = self
            .additions
            .checked_add(additions)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        Ok(())
    }
}

struct FivePointBlockEvaluationV1 {
    plan: FivePointBlockPlanV1,
    values: [u64; REPETITIONS_V1],
    block_powers: [u64; REPETITIONS_V1],
    blocks_absorbed: usize,
    work: EvaluationWorkV1,
}

impl FivePointBlockEvaluationV1 {
    const fn new_v1(plan: FivePointBlockPlanV1) -> Self {
        Self {
            plan,
            values: [0; REPETITIONS_V1],
            block_powers: [1; REPETITIONS_V1],
            blocks_absorbed: 0,
            work: EvaluationWorkV1 {
                coefficients: 0,
                multiplications: 0,
                additions: 0,
            },
        }
    }

    fn absorb_block_v1(
        &mut self,
        encoded: &[u8],
    ) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        let encoded_block_bytes = self
            .plan
            .coefficients_per_block
            .checked_mul(RESIDUE_BYTES_V1)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.blocks_absorbed >= self.plan.block_count || encoded.len() != encoded_block_bytes {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount);
        }
        let mut block_values = [0_u64; REPETITIONS_V1];
        for coefficient in encoded.chunks_exact(RESIDUE_BYTES_V1).rev() {
            let coefficient = u64::from_be_bytes(
                coefficient
                    .try_into()
                    .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::InvalidCount)?,
            );
            if coefficient >= self.plan.modulus {
                return Err(RnsNativePublicPolynomialReaderErrorV1::NonCanonicalCoefficient);
            }
            for (value, point) in block_values.iter_mut().zip(self.plan.points) {
                *value = mod_add_v1(
                    mod_mul_v1(*value, point, self.plan.modulus),
                    coefficient,
                    self.plan.modulus,
                );
            }
            self.work
                .charge_v1(1, REPETITIONS_V1 as u64, REPETITIONS_V1 as u64)?;
        }
        for repetition in 0..REPETITIONS_V1 {
            self.values[repetition] = mod_add_v1(
                self.values[repetition],
                mod_mul_v1(
                    self.block_powers[repetition],
                    block_values[repetition],
                    self.plan.modulus,
                ),
                self.plan.modulus,
            );
        }
        self.work
            .charge_v1(0, REPETITIONS_V1 as u64, REPETITIONS_V1 as u64)?;
        self.blocks_absorbed = self
            .blocks_absorbed
            .checked_add(1)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.blocks_absorbed < self.plan.block_count {
            for repetition in 0..REPETITIONS_V1 {
                self.block_powers[repetition] = mod_mul_v1(
                    self.block_powers[repetition],
                    self.plan.block_steps[repetition],
                    self.plan.modulus,
                );
            }
            self.work.charge_v1(0, REPETITIONS_V1 as u64, 0)?;
        }
        Ok(())
    }

    fn finish_v1(
        self,
    ) -> Result<([u64; REPETITIONS_V1], EvaluationWorkV1), RnsNativePublicPolynomialReaderErrorV1>
    {
        if self.blocks_absorbed != self.plan.block_count
            || self.work.coefficients != self.plan.ring_degree as u64
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete);
        }
        Ok((self.values, self.work))
    }
}

fn mod_add_v1(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_mul_v1(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) * u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_pow_with_work_v1(mut base: u64, mut exponent: u64, modulus: u64) -> (u64, u64) {
    let mut result = 1_u64;
    let mut work = 0_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mod_mul_v1(result, base, modulus);
            work = work.saturating_add(1);
        }
        base = mod_mul_v1(base, base, modulus);
        work = work.saturating_add(1);
        exponent >>= 1;
    }
    (result, work)
}

/// Public values for one `(limb, repetition)` relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub(super) struct RnsNativePublicPolynomialEvaluationV1 {
    pub(super) public_a: u64,
    pub(super) public_b: u64,
    pub(super) ciphertext_c0: [u64; RECORDS_V1],
    pub(super) ciphertext_c1: [u64; RECORDS_V1],
}

impl RnsNativePublicPolynomialEvaluationV1 {
    const UNFILLED: Self = Self {
        public_a: u64::MAX,
        public_b: u64::MAX,
        ciphertext_c0: [u64::MAX; RECORDS_V1],
        ciphertext_c1: [u64::MAX; RECORDS_V1],
    };
}

const PUBLIC_EVALUATION_CACHE_BYTES_V1: usize =
    REPETITIONS_V1 * core::mem::size_of::<RnsNativePublicPolynomialEvaluationV1>();

const _: () = {
    assert!(core::mem::size_of::<RnsNativePublicPolynomialEvaluationV1>() == 704);
    assert!(PUBLIC_EVALUATION_CACHE_BYTES_V1 == 3_520);
};

/// Non-consensus operational receipt for one complete public-manifest pass.
#[allow(
    missing_copy_implementations,
    reason = "the completed read-set receipt is a single-use source-terminal authority"
)]
#[must_use = "the direct source must retain the completed read-set receipt"]
pub(super) struct RnsNativePublicPolynomialReadReceiptV1 {
    manifest_digest: [u8; DIGEST_BYTES_V1],
    qpcs_schedule_digest: [u8; DIGEST_BYTES_V1],
    provider_identity: [u8; DIGEST_BYTES_V1],
    snapshot_identity: [u8; DIGEST_BYTES_V1],
    object_count: u16,
    canonical_bytes: u64,
    coefficient_count: u64,
    modular_multiplications: u64,
    modular_additions: u64,
    read_set_digest: [u8; DIGEST_BYTES_V1],
}

impl RnsNativePublicPolynomialReadReceiptV1 {
    pub(super) const fn manifest_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.manifest_digest
    }

    pub(super) const fn qpcs_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.qpcs_schedule_digest
    }

    pub(super) const fn provider_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.provider_identity
    }

    pub(super) const fn snapshot_identity_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.snapshot_identity
    }

    pub(super) const fn object_count_v1(&self) -> u16 {
        self.object_count
    }

    pub(super) const fn canonical_bytes_v1(&self) -> u64 {
        self.canonical_bytes
    }

    pub(super) const fn coefficient_count_v1(&self) -> u64 {
        self.coefficient_count
    }

    pub(super) const fn modular_multiplications_v1(&self) -> u64 {
        self.modular_multiplications
    }

    pub(super) const fn modular_additions_v1(&self) -> u64 {
        self.modular_additions
    }

    pub(super) const fn read_set_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.read_set_digest
    }
}

/// Move-only, poison-on-failure owner of one complete authenticated public pass.
#[must_use = "dropping the reader before finish cannot mint a read-set receipt"]
pub(super) struct RnsNativePublicPolynomialReaderV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    manifest: RnsNativePublicPolynomialManifestV1,
    provider: P,
    provider_identity: [u8; DIGEST_BYTES_V1],
    snapshot_identity: [u8; DIGEST_BYTES_V1],
    schedule_identity: Option<QpcsScheduleIdentityV1>,
    next_limb: usize,
    next_repetition: usize,
    cache_limb: Option<usize>,
    cache: [RnsNativePublicPolynomialEvaluationV1; REPETITIONS_V1],
    scratch: [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    objects_read: u16,
    canonical_bytes: u64,
    work: EvaluationWorkV1,
    step_multiplications: u64,
    read_set_hash: Keccak256,
    poisoned: bool,
}

impl<P> RnsNativePublicPolynomialReaderV1<P>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
{
    pub(super) fn new(
        manifest: RnsNativePublicPolynomialManifestV1,
        mut provider: P,
    ) -> Result<Self, RnsNativePublicPolynomialReaderErrorV1> {
        manifest.validate_v1()?;
        let provider_identity = provider
            .provider_identity()
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::SourceUnavailable)?;
        let snapshot_identity = provider
            .snapshot_identity()
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::SourceUnavailable)?;
        if provider_identity == [0; DIGEST_BYTES_V1] || snapshot_identity == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativePublicPolynomialReaderErrorV1::SourceUnavailable);
        }
        let mut read_set_hash = Keccak256::new();
        read_set_hash.update(READ_SET_DOMAIN_V1);
        read_set_hash.update(&[VERSION_V1]);
        read_set_hash.update(&manifest.manifest_digest);
        read_set_hash.update(&provider_identity);
        read_set_hash.update(&snapshot_identity);
        read_set_hash.update(&(PUBLIC_POLYNOMIAL_OBJECTS_V1 as u16).to_be_bytes());
        read_set_hash.update(&(READ_LANGUAGE_V1.len() as u16).to_be_bytes());
        read_set_hash.update(READ_LANGUAGE_V1);
        Ok(Self {
            manifest,
            provider,
            provider_identity,
            snapshot_identity,
            schedule_identity: None,
            next_limb: 0,
            next_repetition: 0,
            cache_limb: None,
            cache: [RnsNativePublicPolynomialEvaluationV1::UNFILLED; REPETITIONS_V1],
            scratch: [0; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
            objects_read: 0,
            canonical_bytes: 0,
            work: EvaluationWorkV1::default(),
            step_multiplications: 0,
            read_set_hash,
            poisoned: false,
        })
    }

    pub(super) const fn manifest(&self) -> &RnsNativePublicPolynomialManifestV1 {
        &self.manifest
    }

    pub(super) fn take_next_evaluation_v1(
        &mut self,
        schedule: &RnsNativeQpcsRelationScheduleV1,
        limb: usize,
        repetition: usize,
    ) -> Result<RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReaderErrorV1> {
        self.take_next_evaluation_with_evaluator_v1(
            schedule,
            limb,
            repetition,
            |reader, descriptor, plan| reader.read_and_evaluate_object_v1(descriptor, plan),
        )
    }

    fn take_next_evaluation_with_evaluator_v1<F>(
        &mut self,
        schedule: &RnsNativeQpcsRelationScheduleV1,
        limb: usize,
        repetition: usize,
        mut evaluate: F,
    ) -> Result<RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReaderErrorV1>
    where
        F: FnMut(
            &mut Self,
            RnsNativePublicPolynomialDescriptorV1,
            FivePointBlockPlanV1,
        ) -> Result<[u64; REPETITIONS_V1], RnsNativePublicPolynomialReaderErrorV1>,
    {
        if self.poisoned {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Poisoned);
        }
        // Poison before any provider-controlled call or fallible state change. A
        // caught unwind can never resume a partially authenticated pass.
        self.poisoned = true;
        let result = self.take_next_evaluation_inner_v1(schedule, limb, repetition, &mut evaluate);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn take_next_evaluation_inner_v1<F>(
        &mut self,
        schedule: &RnsNativeQpcsRelationScheduleV1,
        limb: usize,
        repetition: usize,
        evaluate: &mut F,
    ) -> Result<RnsNativePublicPolynomialEvaluationV1, RnsNativePublicPolynomialReaderErrorV1>
    where
        F: FnMut(
            &mut Self,
            RnsNativePublicPolynomialDescriptorV1,
            FivePointBlockPlanV1,
        ) -> Result<[u64; REPETITIONS_V1], RnsNativePublicPolynomialReaderErrorV1>,
    {
        if limb != self.next_limb
            || repetition != self.next_repetition
            || limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || repetition >= REPETITIONS_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder);
        }
        let schedule_identity = QpcsScheduleIdentityV1::from_schedule_v1(schedule)?;
        match self.schedule_identity {
            None => {
                self.read_set_hash.update(&schedule_identity.binding_digest);
                self.schedule_identity = Some(schedule_identity);
            }
            Some(expected) if expected == schedule_identity => {}
            Some(_) => return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule),
        }
        if repetition == 0 {
            self.prepare_limb_with_evaluator_v1(schedule, limb, evaluate)?;
        }
        if self.cache_limb != Some(limb) {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete);
        }
        let point = schedule
            .point(limb, repetition)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)?;
        if point == 0 || point >= ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb] {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule);
        }
        let value = self.cache[repetition];
        self.next_repetition = self
            .next_repetition
            .checked_add(1)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.next_repetition == REPETITIONS_V1 {
            self.cache
                .fill(RnsNativePublicPolynomialEvaluationV1::UNFILLED);
            self.cache_limb = None;
            self.next_repetition = 0;
            self.next_limb = self
                .next_limb
                .checked_add(1)
                .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        }
        Ok(value)
    }

    fn prepare_limb_with_evaluator_v1<F>(
        &mut self,
        schedule: &RnsNativeQpcsRelationScheduleV1,
        limb: usize,
        evaluate: &mut F,
    ) -> Result<(), RnsNativePublicPolynomialReaderErrorV1>
    where
        F: FnMut(
            &mut Self,
            RnsNativePublicPolynomialDescriptorV1,
            FivePointBlockPlanV1,
        ) -> Result<[u64; REPETITIONS_V1], RnsNativePublicPolynomialReaderErrorV1>,
    {
        if self.cache_limb.is_some() || limb != self.next_limb || self.next_repetition != 0 {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder);
        }
        let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
        let mut points = [0_u64; REPETITIONS_V1];
        for (repetition, point) in points.iter_mut().enumerate() {
            *point = schedule
                .point(limb, repetition)
                .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidSchedule)?;
        }
        let plan = FivePointBlockPlanV1::new_v1(
            modulus,
            points,
            ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
            COEFFICIENTS_PER_READ_V1,
        )?;
        if plan.step_multiplications
            != REPETITIONS_V1 as u64 * BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow);
        }
        self.step_multiplications = self
            .step_multiplications
            .checked_add(plan.step_multiplications)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.step_multiplications
            > ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u64
                * REPETITIONS_V1 as u64
                * BLOCK_STEP_MULTIPLICATIONS_PER_POINT_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ResourceCeilingExceeded);
        }
        // `self.cache` is the only five-evaluation array. It remains hidden by
        // `cache_limb == None` until all 88 objects authenticate; any failure
        // poisons the reader permanently, so partially filled values cannot
        // escape and no second 3,520-byte cache is needed.
        self.cache
            .fill(RnsNativePublicPolynomialEvaluationV1::UNFILLED);
        let public_a =
            self.required_descriptor_v1(RnsNativePublicPolynomialRoleV1::PublicA, None, limb)?;
        let values = evaluate(self, public_a, plan)?;
        for repetition in 0..REPETITIONS_V1 {
            self.cache[repetition].public_a = values[repetition];
        }
        let public_b =
            self.required_descriptor_v1(RnsNativePublicPolynomialRoleV1::PublicB, None, limb)?;
        let values = evaluate(self, public_b, plan)?;
        for repetition in 0..REPETITIONS_V1 {
            self.cache[repetition].public_b = values[repetition];
        }
        for record in 0..RECORDS_V1 {
            let c0 = self.required_descriptor_v1(
                RnsNativePublicPolynomialRoleV1::CiphertextC0,
                Some(record),
                limb,
            )?;
            let values = evaluate(self, c0, plan)?;
            for repetition in 0..REPETITIONS_V1 {
                self.cache[repetition].ciphertext_c0[record] = values[repetition];
            }
            let c1 = self.required_descriptor_v1(
                RnsNativePublicPolynomialRoleV1::CiphertextC1,
                Some(record),
                limb,
            )?;
            let values = evaluate(self, c1, plan)?;
            for repetition in 0..REPETITIONS_V1 {
                self.cache[repetition].ciphertext_c1[record] = values[repetition];
            }
        }
        if self.cache.iter().any(|evaluation| {
            evaluation.public_a == u64::MAX
                || evaluation.public_b == u64::MAX
                || evaluation.ciphertext_c0.contains(&u64::MAX)
                || evaluation.ciphertext_c1.contains(&u64::MAX)
        }) {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete);
        }
        self.cache_limb = Some(limb);
        Ok(())
    }

    fn required_descriptor_v1(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Result<RnsNativePublicPolynomialDescriptorV1, RnsNativePublicPolynomialReaderErrorV1> {
        self.manifest
            .descriptor_for_runtime_read_v1(role, record, limb)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::InvalidOrder)
    }

    #[cfg(test)]
    fn take_one_object_for_test_v1(
        &mut self,
        descriptor: RnsNativePublicPolynomialDescriptorV1,
        plan: FivePointBlockPlanV1,
    ) -> Result<[u64; REPETITIONS_V1], RnsNativePublicPolynomialReaderErrorV1> {
        if self.poisoned {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Poisoned);
        }
        self.poisoned = true;
        let result = self.read_and_evaluate_object_v1(descriptor, plan);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn read_and_evaluate_object_v1(
        &mut self,
        descriptor: RnsNativePublicPolynomialDescriptorV1,
        plan: FivePointBlockPlanV1,
    ) -> Result<[u64; REPETITIONS_V1], RnsNativePublicPolynomialReaderErrorV1> {
        descriptor.validate_v1()?;
        let next_bytes = self
            .canonical_bytes
            .checked_add(LIMB_OBJECT_BYTES_V1)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if next_bytes > PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1
            || next_bytes > ZK_AMS_MKHE_RNS_NATIVE_IO_MAX_BYTES_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ResourceCeilingExceeded);
        }
        let mut transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            descriptor.role.object_kind_v1(),
            descriptor.pointer,
            &mut self.provider,
        )
        .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::Authentication)?;
        let mut count = [0_u8; LIMB_COUNT_PREFIX_BYTES_V1];
        if transaction
            .read_next(&mut self.provider, &mut count)
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::Authentication)?
            != count.len()
            || usize::try_from(u32::from_be_bytes(count)).ok()
                != Some(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount);
        }
        let mut evaluation = FivePointBlockEvaluationV1::new_v1(plan);
        for _ in 0..COEFFICIENT_READS_PER_OBJECT_V1 {
            let read = transaction
                .read_next(&mut self.provider, &mut self.scratch)
                .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::Authentication)?;
            if read != self.scratch.len() {
                return Err(RnsNativePublicPolynomialReaderErrorV1::InvalidCount);
            }
            evaluation.absorb_block_v1(&self.scratch[..read])?;
        }
        if transaction.remaining_bytes() != 0 {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete);
        }
        let receipt = transaction
            .finish(&mut self.provider)
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::Authentication)?;
        self.validate_and_absorb_receipt_v1(descriptor, &receipt)?;
        let (values, work) = evaluation.finish_v1()?;
        if work.coefficients != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
            || work.multiplications != MODULAR_MULTIPLICATIONS_PER_OBJECT_V1
            || work.additions != MODULAR_ADDITIONS_PER_OBJECT_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow);
        }
        self.work
            .charge_v1(work.coefficients, work.multiplications, work.additions)?;
        let total_multiplications = self
            .work
            .multiplications
            .checked_add(self.step_multiplications)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.work.coefficients > PUBLIC_POLYNOMIAL_COEFFICIENTS_V1
            || total_multiplications > PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1
            || self.work.additions > PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ResourceCeilingExceeded);
        }
        self.canonical_bytes = next_bytes;
        Ok(values)
    }

    fn validate_and_absorb_receipt_v1(
        &mut self,
        descriptor: RnsNativePublicPolynomialDescriptorV1,
        receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    ) -> Result<(), RnsNativePublicPolynomialReaderErrorV1> {
        let snapshot = receipt.snapshot();
        if snapshot.provider_identity() != self.provider_identity
            || snapshot.snapshot_identity() != self.snapshot_identity
            || snapshot.pointer() != descriptor.pointer
            || receipt.canonical_bytes() != LIMB_OBJECT_BYTES_V1
            || receipt.payload_blake3() != descriptor.pointer.payload_blake3()
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Authentication);
        }
        let ordinal = self.objects_read;
        self.objects_read = self
            .objects_read
            .checked_add(1)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if usize::from(self.objects_read) > PUBLIC_POLYNOMIAL_OBJECTS_V1 {
            return Err(RnsNativePublicPolynomialReaderErrorV1::ResourceCeilingExceeded);
        }
        self.read_set_hash.update(&ordinal.to_be_bytes());
        self.read_set_hash.update(&[
            descriptor.role as u8,
            descriptor.record.unwrap_or(u8::MAX),
            descriptor.limb,
        ]);
        self.read_set_hash.update(&descriptor.artifact_digest);
        self.read_set_hash
            .update(&descriptor.pointer.pointer_digest());
        self.read_set_hash.update(&receipt.receipt_digest());
        Ok(())
    }

    pub(super) fn finish(
        mut self,
    ) -> Result<RnsNativePublicPolynomialReadReceiptV1, RnsNativePublicPolynomialReaderErrorV1>
    {
        if self.poisoned {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Poisoned);
        }
        let total_multiplications = self
            .work
            .multiplications
            .checked_add(self.step_multiplications)
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::ArithmeticOverflow)?;
        if self.next_limb != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || self.next_repetition != 0
            || self.cache_limb.is_some()
            || usize::from(self.objects_read) != PUBLIC_POLYNOMIAL_OBJECTS_V1
            || self.canonical_bytes != PUBLIC_POLYNOMIAL_CANONICAL_BYTES_V1
            || self.work.coefficients != PUBLIC_POLYNOMIAL_COEFFICIENTS_V1
            || total_multiplications != PUBLIC_POLYNOMIAL_MODULAR_MULTIPLICATIONS_V1
            || self.work.additions != PUBLIC_POLYNOMIAL_MODULAR_ADDITIONS_V1
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Incomplete);
        }
        let schedule_identity = self
            .schedule_identity
            .ok_or(RnsNativePublicPolynomialReaderErrorV1::Incomplete)?;
        schedule_identity.validate_v1()?;
        let provider_identity = self
            .provider
            .provider_identity()
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::SourceUnavailable)?;
        let snapshot_identity = self
            .provider
            .snapshot_identity()
            .map_err(|_| RnsNativePublicPolynomialReaderErrorV1::SourceUnavailable)?;
        if provider_identity != self.provider_identity
            || snapshot_identity != self.snapshot_identity
        {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Authentication);
        }
        self.read_set_hash.update(&self.objects_read.to_be_bytes());
        self.read_set_hash
            .update(&self.canonical_bytes.to_be_bytes());
        self.read_set_hash
            .update(&self.work.coefficients.to_be_bytes());
        self.read_set_hash
            .update(&total_multiplications.to_be_bytes());
        self.read_set_hash
            .update(&self.work.additions.to_be_bytes());
        let read_set_digest = self.read_set_hash.finalize();
        if read_set_digest == [0; DIGEST_BYTES_V1] {
            return Err(RnsNativePublicPolynomialReaderErrorV1::Authentication);
        }
        Ok(RnsNativePublicPolynomialReadReceiptV1 {
            manifest_digest: self.manifest.manifest_digest,
            qpcs_schedule_digest: schedule_identity.binding_digest,
            provider_identity: self.provider_identity,
            snapshot_identity: self.snapshot_identity,
            object_count: self.objects_read,
            canonical_bytes: self.canonical_bytes,
            coefficient_count: self.work.coefficients,
            modular_multiplications: total_multiplications,
            modular_additions: self.work.additions,
            read_set_digest,
        })
    }
}

#[cfg(test)]
#[path = "rns_native_public_polynomial_reader_tests.rs"]
mod tests;
