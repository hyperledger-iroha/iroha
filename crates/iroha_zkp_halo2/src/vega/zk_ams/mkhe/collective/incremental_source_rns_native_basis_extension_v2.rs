//! Private, non-authorizing 38-to-40-limb basis-extension arithmetic.
//!
//! This privately declared, non-authorizing child specifies only the additive
//! V2 arithmetic boundary needed to preserve every frozen release-38 object while
//! computing the two genuine replacement-profile tail residues. It does not
//! publish an object, construct a source adapter, mint a receipt/capability, or
//! authorize the 40-limb profile. In particular, it never reconstructs a tail
//! from the 38-limb prefix: `A[38..40]` is independently domain-separated,
//! `B[38..40]` is computed only from eight V1-admitted `s,e` states, and each
//! `C[38..40]` is computed only while the V1 encryption callback still borrows
//! the exact `m,r,e0,e1` opening.
//!
//! This module is declared as a child of `collective::incremental_source`.
//! That placement is part of the contract:
//! it grants access to the existing two-limb negacyclic kernel and lets only
//! the parent encryption implementation construct the synchronous opening.
//! The 43-record lifecycle added here returns only a non-authorizing checksum.
//! Its private tail-publication sibling consumes the arithmetic owner into
//! actual CAS receipts and an opaque whole-V1 owner contract. The compiled V1
//! coordinator now allocates the workspace in the validated pre-entropy
//! factory and moves it through the synchronous callback. This source-level
//! chronology does not provide the still-absent live Phase-23 owner, source
//! adapter, evidence, readiness, or release authority.

#![allow(
    dead_code,
    reason = "the private non-authorizing V2 arithmetic contract has no live Phase-23 caller"
)]

use super::super::super::{
    PlaintextModulus, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    bytes_mod_u64,
    cpk_ceremony::ZkAmsMkheAdmittedCpkPartyV1,
    direct_object_transport::ZkAmsMkheDirectObjectKindV1,
    manifest::{
        RELEASE_MODULI_V1, RELEASE_NEGACYCLIC_ROOTS_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1,
    },
    mod_add, mod_mul,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1, zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_profile_v1,
    },
    signed_mod, t256_centered_residue_with_modulus_residue,
};
use super::super::release_security_certificate_digest;
use super::{
    ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    negacyclic_multiply_signed_rhs_two_limb_v1,
};
use crate::vega::{
    VEGA_T256_SCALAR_MODULUS_BE_V1,
    sponge::{Keccak256, Shake256Reader},
};

const BASIS_EXTENSION_VERSION_V2: u8 = 2;
const LEGACY_LIMB_COUNT_V2: usize = RELEASE_MODULI_V1.len();
const TARGET_LIMB_COUNT_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
const TAIL_LIMB_COUNT_V2: usize = TARGET_LIMB_COUNT_V2 - LEGACY_LIMB_COUNT_V2;
const PUBLIC_POLYNOMIAL_ROLE_COUNT_V2: usize = 2 + 2 * RECORD_COUNT_V2;
const RECORD_COUNT_V2: usize = 43;
const FULL_OBJECT_COUNT_V2: usize = PUBLIC_POLYNOMIAL_ROLE_COUNT_V2 * TARGET_LIMB_COUNT_V2;
const LEGACY_OBJECT_COUNT_V2: usize = PUBLIC_POLYNOMIAL_ROLE_COUNT_V2 * LEGACY_LIMB_COUNT_V2;
const MISSING_TAIL_OBJECT_COUNT_V2: usize = FULL_OBJECT_COUNT_V2 - LEGACY_OBJECT_COUNT_V2;
const KEY_TAIL_OBJECT_COUNT_V2: usize = 2 * TAIL_LIMB_COUNT_V2;
const CIPHERTEXT_TAIL_OBJECT_COUNT_V2: usize = 2 * RECORD_COUNT_V2 * TAIL_LIMB_COUNT_V2;
const COEFFICIENTS_PER_CHUNK_V2: usize = 1_024;
const CHUNK_COUNT_PER_OBJECT_V2: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / COEFFICIENTS_PER_CHUNK_V2;
const COUNT_PREFIX_BYTES_V2: usize = 4;
const CHUNK_BYTES_V2: usize = COEFFICIENTS_PER_CHUNK_V2 * core::mem::size_of::<u64>();
const OBJECT_BYTES_V2: usize =
    COUNT_PREFIX_BYTES_V2 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const TAIL_COEFFICIENT_COUNT_V2: u64 =
    MISSING_TAIL_OBJECT_COUNT_V2 as u64 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64;
const TAIL_CAS_BYTES_V2: u64 = MISSING_TAIL_OBJECT_COUNT_V2 as u64 * OBJECT_BYTES_V2 as u64;
const B_TAIL_NEGACYCLIC_PRODUCTS_V2: u64 =
    (ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * TAIL_LIMB_COUNT_V2) as u64;
const C_TAIL_NEGACYCLIC_PRODUCTS_V2: u64 = (RECORD_COUNT_V2 * 2 * TAIL_LIMB_COUNT_V2) as u64;
const TOTAL_TAIL_NEGACYCLIC_PRODUCTS_V2: u64 =
    B_TAIL_NEGACYCLIC_PRODUCTS_V2 + C_TAIL_NEGACYCLIC_PRODUCTS_V2;
const WORK_UNITS_PER_NEGACYCLIC_PRODUCT_V2: u64 = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
    * (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1.trailing_zeros() as u64 + 1);
const TOTAL_TAIL_NEGACYCLIC_WORK_UNITS_V2: u64 =
    TOTAL_TAIL_NEGACYCLIC_PRODUCTS_V2 * WORK_UNITS_PER_NEGACYCLIC_PRODUCT_V2;
const PUBLIC_KEY_TAIL_COEFFICIENT_BYTES_V2: usize =
    KEY_TAIL_OBJECT_COUNT_V2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const TWO_LIMB_WORKSPACE_BYTES_V2: usize =
    2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const BUILDER_NAMED_HEAP_PEAK_BYTES_V2: usize =
    PUBLIC_KEY_TAIL_COEFFICIENT_BYTES_V2 + TWO_LIMB_WORKSPACE_BYTES_V2;
const SYNCHRONOUS_SAME_OPENING_BORROWED_BYTES_V2: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    * (core::mem::size_of::<[u8; 32]>() + 3 * core::mem::size_of::<i64>())
    + core::mem::size_of::<[u8; 32]>();
const PARTY_CONTRIBUTION_DIGEST_BYTES_V2: usize =
    ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * core::mem::size_of::<[u8; 32]>();
const MAX_PUBLIC_A_CANDIDATES_PER_COEFFICIENT_V2: usize = 128;
const MAX_PUBLIC_A_TAIL_CANDIDATES_V2: u64 = (TAIL_LIMB_COUNT_V2
    * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
    * MAX_PUBLIC_A_CANDIDATES_PER_COEFFICIENT_V2)
    as u64;
const MAX_PUBLIC_A_TAIL_XOF_BYTES_V2: u64 =
    MAX_PUBLIC_A_TAIL_CANDIDATES_V2 * core::mem::size_of::<u64>() as u64;

const PUBLIC_A_TAIL_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.public-a-tail";
const BASIS_EXTENSION_CONTRACT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.contract";
const KEY_TAIL_INTEGRITY_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.key-tail-integrity";
const PARTY_B_TAIL_CONTRIBUTION_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.party-b-tail";
const CIPHERTEXT_TAIL_COMPLETION_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.ciphertext-tail";
const CIPHERTEXT_TAIL_LIFECYCLE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.rns-native-public-polynomial.basis-extension.ciphertext-tail-lifecycle";
const CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2: u8 = (2 * TAIL_LIMB_COUNT_V2) as u8;

const PUBLIC_A_TAIL_FRAME_BYTES_V2: usize =
    PUBLIC_A_TAIL_DOMAIN_V2.len() + 1 + 32 + 32 + 32 + 32 + 32 + 8 + 32 + 4 + 2 + 2 + 2 + 8 + 8;

/// This source-only tranche implements arithmetic, never a production owner.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_CONTRACT_IMPLEMENTED_V2: bool = true;
/// The compiled V1 coordinator allocates each record workspace in the
/// validated pre-entropy factory. This is a source contract, not liveness.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_PRE_ENTROPY_CALL_SITE_INTEGRATED_V2: bool = true;
/// The frozen V1 prefix must be retained byte-for-byte, never rederived here.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_PREFIX_RECOMPUTATION_ALLOWED_V2: bool = false;
/// No CRT interpolation, digest synthesis, or other fake tail lift is allowed.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_FAKE_TAIL_LIFT_ALLOWED_V2: bool = false;
/// There is deliberately no governed production owner in this tranche.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_PRODUCTION_OWNER_AVAILABLE_V2: bool = false;
/// There is deliberately no public-polynomial source adapter in this tranche.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_SOURCE_ADAPTER_AVAILABLE_V2: bool = false;
/// The compiled V1 coordinator invokes this private arithmetic module. The
/// Phase-23/source backend remains unavailable.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_INTEGRATED_V2: bool = true;
/// The open 40-limb evidence pins remain non-authorizing.
pub(super) const RNS_NATIVE_BASIS_EXTENSION_RELEASE_AUTHORIZED_V2: bool = false;

const _: () = {
    assert!(LEGACY_LIMB_COUNT_V2 == 38);
    assert!(TARGET_LIMB_COUNT_V2 == 40);
    assert!(TAIL_LIMB_COUNT_V2 == 2);
    assert!(PUBLIC_POLYNOMIAL_ROLE_COUNT_V2 == 88);
    assert!(FULL_OBJECT_COUNT_V2 == 3_520);
    assert!(LEGACY_OBJECT_COUNT_V2 == 3_344);
    assert!(MISSING_TAIL_OBJECT_COUNT_V2 == 176);
    assert!(KEY_TAIL_OBJECT_COUNT_V2 == 4);
    assert!(CIPHERTEXT_TAIL_OBJECT_COUNT_V2 == 172);
    assert!(CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2 == 4);
    assert!(
        RECORD_COUNT_V2 * CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2 as usize
            == CIPHERTEXT_TAIL_OBJECT_COUNT_V2
    );
    assert!(CHUNK_COUNT_PER_OBJECT_V2 == 128);
    assert!(CHUNK_BYTES_V2 == 8_192);
    assert!(OBJECT_BYTES_V2 == 1_048_580);
    assert!(TAIL_COEFFICIENT_COUNT_V2 == 23_068_672);
    assert!(TAIL_CAS_BYTES_V2 == 184_550_080);
    assert!(B_TAIL_NEGACYCLIC_PRODUCTS_V2 == 16);
    assert!(C_TAIL_NEGACYCLIC_PRODUCTS_V2 == 172);
    assert!(TOTAL_TAIL_NEGACYCLIC_PRODUCTS_V2 == 188);
    assert!(WORK_UNITS_PER_NEGACYCLIC_PRODUCT_V2 == 2_359_296);
    assert!(TOTAL_TAIL_NEGACYCLIC_WORK_UNITS_V2 == 443_547_648);
    assert!(PUBLIC_KEY_TAIL_COEFFICIENT_BYTES_V2 == 4_194_304);
    assert!(TWO_LIMB_WORKSPACE_BYTES_V2 == 2_097_152);
    assert!(BUILDER_NAMED_HEAP_PEAK_BYTES_V2 == 6_291_456);
    assert!(SYNCHRONOUS_SAME_OPENING_BORROWED_BYTES_V2 == 7_340_064);
    assert!(PARTY_CONTRIBUTION_DIGEST_BYTES_V2 == 256);
    assert!(MAX_PUBLIC_A_TAIL_CANDIDATES_V2 == 33_554_432);
    assert!(MAX_PUBLIC_A_TAIL_XOF_BYTES_V2 == 268_435_456);
    assert!(RNS_NATIVE_BASIS_EXTENSION_CONTRACT_IMPLEMENTED_V2);
    assert!(RNS_NATIVE_BASIS_EXTENSION_PRE_ENTROPY_CALL_SITE_INTEGRATED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_PREFIX_RECOMPUTATION_ALLOWED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_FAKE_TAIL_LIFT_ALLOWED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_PRODUCTION_OWNER_AVAILABLE_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_SOURCE_ADAPTER_AVAILABLE_V2);
    assert!(RNS_NATIVE_BASIS_EXTENSION_INTEGRATED_V2);
    assert!(!RNS_NATIVE_BASIS_EXTENSION_RELEASE_AUTHORIZED_V2);
};

/// Closed error vocabulary for the private arithmetic contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeBasisExtensionErrorV2 {
    Dependency,
    InvalidProfileExtension,
    InvalidAxes,
    InvalidPartyOrder,
    InvalidAdmittedState,
    InvalidSameOpening,
    InvalidRecordOrder,
    InvalidRecordCompletion,
    InvalidPosition,
    InvalidResidue,
    ResourceCeiling,
    IncompleteObject,
    IncompleteLifecycle,
    VisitorRejected,
    Poisoned,
}

fn dependency_v2<T>(
    result: Result<T, ZkAmsMkheErrorV1>,
) -> Result<T, RnsNativeBasisExtensionErrorV2> {
    result.map_err(|_| RnsNativeBasisExtensionErrorV2::Dependency)
}

/// Exact static accounting for this additive tail only.
///
/// `builder_named_heap_peak_bytes` counts the four retained public key-tail
/// limbs plus one reused two-limb arithmetic workspace. It excludes the
/// caller-owned V1 admitted states, source owner, visitor storage, CAS backend,
/// allocator metadata, and the caller-supplied 8 KiB encoder scratch.
/// `synchronous_same_opening_borrowed_bytes` is the exact callback payload
/// borrowed from V1, not newly allocated or retained by this module.
/// `ciphertext_workspace_required_pre_entropy_bytes` is discharged by the
/// compiled parent coordinator. This source API also proves that moving the
/// allocated owner into the live kernel allocates zero additional bytes. The
/// separate Phase-23/liveness and measured-evidence claims remain open.
pub(super) struct RnsNativeBasisExtensionResourcesV2 {
    pub(super) full_object_count: u32,
    pub(super) legacy_object_count: u32,
    pub(super) missing_tail_object_count: u16,
    pub(super) chunks_per_object: u16,
    pub(super) object_bytes: u32,
    pub(super) tail_coefficient_count: u64,
    pub(super) tail_cas_bytes: u64,
    pub(super) b_tail_negacyclic_products: u16,
    pub(super) c_tail_negacyclic_products: u16,
    pub(super) total_tail_negacyclic_work_units: u64,
    pub(super) public_key_tail_coefficient_bytes: u32,
    pub(super) two_limb_workspace_bytes: u32,
    pub(super) builder_named_heap_peak_bytes: u32,
    pub(super) ciphertext_kernel_owned_heap_bytes: u32,
    pub(super) ciphertext_workspace_required_pre_entropy_bytes: u32,
    pub(super) ciphertext_kernel_constructor_allocation_bytes: u32,
    pub(super) key_owner_plus_kernel_resident_bytes: u32,
    pub(super) synchronous_same_opening_borrowed_bytes: u32,
    pub(super) encoder_scratch_bytes: u16,
    pub(super) public_a_frame_stack_bytes: u16,
    pub(super) party_contribution_digest_bytes: u16,
    pub(super) max_public_a_tail_candidates: u64,
    pub(super) max_public_a_tail_xof_bytes: u64,
}

pub(super) const RNS_NATIVE_BASIS_EXTENSION_RESOURCES_V2: RnsNativeBasisExtensionResourcesV2 =
    RnsNativeBasisExtensionResourcesV2 {
        full_object_count: FULL_OBJECT_COUNT_V2 as u32,
        legacy_object_count: LEGACY_OBJECT_COUNT_V2 as u32,
        missing_tail_object_count: MISSING_TAIL_OBJECT_COUNT_V2 as u16,
        chunks_per_object: CHUNK_COUNT_PER_OBJECT_V2 as u16,
        object_bytes: OBJECT_BYTES_V2 as u32,
        tail_coefficient_count: TAIL_COEFFICIENT_COUNT_V2,
        tail_cas_bytes: TAIL_CAS_BYTES_V2,
        b_tail_negacyclic_products: B_TAIL_NEGACYCLIC_PRODUCTS_V2 as u16,
        c_tail_negacyclic_products: C_TAIL_NEGACYCLIC_PRODUCTS_V2 as u16,
        total_tail_negacyclic_work_units: TOTAL_TAIL_NEGACYCLIC_WORK_UNITS_V2,
        public_key_tail_coefficient_bytes: PUBLIC_KEY_TAIL_COEFFICIENT_BYTES_V2 as u32,
        two_limb_workspace_bytes: TWO_LIMB_WORKSPACE_BYTES_V2 as u32,
        builder_named_heap_peak_bytes: BUILDER_NAMED_HEAP_PEAK_BYTES_V2 as u32,
        ciphertext_kernel_owned_heap_bytes: TWO_LIMB_WORKSPACE_BYTES_V2 as u32,
        ciphertext_workspace_required_pre_entropy_bytes: TWO_LIMB_WORKSPACE_BYTES_V2 as u32,
        ciphertext_kernel_constructor_allocation_bytes: 0,
        key_owner_plus_kernel_resident_bytes: BUILDER_NAMED_HEAP_PEAK_BYTES_V2 as u32,
        synchronous_same_opening_borrowed_bytes: SYNCHRONOUS_SAME_OPENING_BORROWED_BYTES_V2 as u32,
        encoder_scratch_bytes: CHUNK_BYTES_V2 as u16,
        public_a_frame_stack_bytes: PUBLIC_A_TAIL_FRAME_BYTES_V2 as u16,
        party_contribution_digest_bytes: PARTY_CONTRIBUTION_DIGEST_BYTES_V2 as u16,
        max_public_a_tail_candidates: MAX_PUBLIC_A_TAIL_CANDIDATES_V2,
        max_public_a_tail_xof_bytes: MAX_PUBLIC_A_TAIL_XOF_BYTES_V2,
    };

/// Typed immutable axes inherited from the governed V1 roster and CPK.
///
/// There is no constructor from bare digests. Tests may assemble a private
/// fixture because their child module can see fields, but production callers
/// can obtain this value only through [`Self::from_v1_governed_context_v2`].
pub(super) struct RnsNativeBasisExtensionAxesV2 {
    release_profile_digest: [u8; 32],
    target_profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    parties: [ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
}

impl RnsNativeBasisExtensionAxesV2 {
    pub(super) fn from_v1_governed_context_v2(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        cpk_transcript_digest: [u8; 32],
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        dependency_v2(roster.validate())?;
        if cpk_transcript_digest == [0; 32] {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAxes);
        }
        let release_profile = release_profile_v1();
        dependency_v2(release_profile.validate())?;
        let release_profile_digest = dependency_v2(release_profile.digest())?;
        if roster.profile_digest() != release_profile_digest {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAxes);
        }
        let target_profile = dependency_v2(zk_ams_mkhe_rns_native_profile_v1())?;
        let axes = Self {
            release_profile_digest,
            target_profile_digest: target_profile.profile_digest,
            security_certificate_digest: dependency_v2(release_security_certificate_digest())?,
            roster_digest: roster.roster_digest(),
            key_material_digest: roster.key_material_digest(),
            epoch: roster.epoch(),
            cpk_transcript_digest,
            parties: (*roster.participants()).map(|participant| participant.party()),
        };
        axes.validate_v2()?;
        Ok(axes)
    }

    fn validate_v2(&self) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        validate_profile_prefix_extension_v2()?;
        let release_profile = release_profile_v1();
        let target_profile = dependency_v2(zk_ams_mkhe_rns_native_profile_v1())?;
        if self.release_profile_digest != dependency_v2(release_profile.digest())?
            || self.target_profile_digest != target_profile.profile_digest
            || self.security_certificate_digest
                != dependency_v2(release_security_certificate_digest())?
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.cpk_transcript_digest == [0; 32]
            || self.parties.iter().any(|party| party.to_bytes() == [0; 32])
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAxes);
        }
        Ok(())
    }
}

fn validate_profile_prefix_extension_v2() -> Result<(), RnsNativeBasisExtensionErrorV2> {
    let release_profile = release_profile_v1();
    dependency_v2(release_profile.validate())?;
    let target_profile = dependency_v2(zk_ams_mkhe_rns_native_profile_v1())?;
    let target_manifest = dependency_v2(zk_ams_mkhe_rns_native_profile_manifest_v1())?;
    dependency_v2(target_manifest.validate())?;
    if release_profile.ring_degree != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        || release_profile.moduli != RELEASE_MODULI_V1
        || release_profile.negacyclic_roots != RELEASE_NEGACYCLIC_ROOTS_V1
        || release_profile.plaintext_modulus != PlaintextModulus::T256
        || release_profile.error_eta != 2
        || target_profile.ring_degree as usize != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        || target_profile.roster_size as usize != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || target_profile.rns_limb_count as usize != TARGET_LIMB_COUNT_V2
        || target_profile.evidence_complete()
        || target_profile.security_certificate_digest != [0; 32]
        || target_profile.release_kat_digest != [0; 32]
        || target_profile.resource_evidence_digest != [0; 32]
        || target_manifest.authorizes_release()
        || ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[..LEGACY_LIMB_COUNT_V2] != RELEASE_MODULI_V1
        || ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[..LEGACY_LIMB_COUNT_V2]
            != RELEASE_NEGACYCLIC_ROOTS_V1
        || ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[38] != 1_152_921_504_403_947_521
        || ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[39] != 1_152_921_504_396_869_633
        || ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[38] != 22_173_257_170_052_426
        || ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[39] != 24_990_432_311_765_759
    {
        return Err(RnsNativeBasisExtensionErrorV2::InvalidProfileExtension);
    }
    Ok(())
}

fn basis_extension_contract_digest_v2() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(BASIS_EXTENSION_CONTRACT_DOMAIN_V2);
    hash.update(&[BASIS_EXTENSION_VERSION_V2]);
    for value in [
        LEGACY_LIMB_COUNT_V2 as u64,
        TARGET_LIMB_COUNT_V2 as u64,
        RECORD_COUNT_V2 as u64,
        FULL_OBJECT_COUNT_V2 as u64,
        LEGACY_OBJECT_COUNT_V2 as u64,
        MISSING_TAIL_OBJECT_COUNT_V2 as u64,
        COEFFICIENTS_PER_CHUNK_V2 as u64,
        CHUNK_COUNT_PER_OBJECT_V2 as u64,
        OBJECT_BYTES_V2 as u64,
        TAIL_COEFFICIENT_COUNT_V2,
        TAIL_CAS_BYTES_V2,
        TOTAL_TAIL_NEGACYCLIC_PRODUCTS_V2,
        TOTAL_TAIL_NEGACYCLIC_WORK_UNITS_V2,
        TWO_LIMB_WORKSPACE_BYTES_V2 as u64,
        BUILDER_NAMED_HEAP_PEAK_BYTES_V2 as u64,
        SYNCHRONOUS_SAME_OPENING_BORROWED_BYTES_V2 as u64,
        CHUNK_BYTES_V2 as u64,
        PUBLIC_A_TAIL_FRAME_BYTES_V2 as u64,
        PARTY_CONTRIBUTION_DIGEST_BYTES_V2 as u64,
        MAX_PUBLIC_A_TAIL_CANDIDATES_V2,
        MAX_PUBLIC_A_TAIL_XOF_BYTES_V2,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for (&modulus, &root) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .iter()
        .zip(ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1.iter())
    {
        hash.update(&modulus.to_be_bytes());
        hash.update(&root.to_be_bytes());
    }
    hash.update(&[
        RNS_NATIVE_BASIS_EXTENSION_CONTRACT_IMPLEMENTED_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_PRE_ENTROPY_CALL_SITE_INTEGRATED_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_PREFIX_RECOMPUTATION_ALLOWED_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_FAKE_TAIL_LIFT_ALLOWED_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_PRODUCTION_OWNER_AVAILABLE_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_SOURCE_ADAPTER_AVAILABLE_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_INTEGRATED_V2.into(),
        RNS_NATIVE_BASIS_EXTENSION_RELEASE_AUTHORIZED_V2.into(),
    ]);
    hash.finalize()
}

fn write_frame_part_v2<const N: usize>(
    frame: &mut [u8; N],
    cursor: &mut usize,
    bytes: &[u8],
) -> Result<(), RnsNativeBasisExtensionErrorV2> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
    let destination = frame
        .get_mut(*cursor..end)
        .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
    destination.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn public_a_tail_frame_v2(
    axes: &RnsNativeBasisExtensionAxesV2,
    limb: usize,
) -> Result<[u8; PUBLIC_A_TAIL_FRAME_BYTES_V2], RnsNativeBasisExtensionErrorV2> {
    if !(LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2).contains(&limb)
        || axes.release_profile_digest == [0; 32]
        || axes.target_profile_digest == [0; 32]
        || axes.security_certificate_digest == [0; 32]
        || axes.roster_digest == [0; 32]
        || axes.key_material_digest == [0; 32]
        || axes.epoch == 0
        || axes.cpk_transcript_digest == [0; 32]
    {
        return Err(RnsNativeBasisExtensionErrorV2::InvalidAxes);
    }
    let ring_degree = u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?
        .to_be_bytes();
    let legacy_limbs = u16::try_from(LEGACY_LIMB_COUNT_V2)
        .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?
        .to_be_bytes();
    let target_limbs = u16::try_from(TARGET_LIMB_COUNT_V2)
        .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?
        .to_be_bytes();
    let limb_bytes = u16::try_from(limb)
        .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?
        .to_be_bytes();
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb].to_be_bytes();
    let root = ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb].to_be_bytes();
    let version = [BASIS_EXTENSION_VERSION_V2];
    let epoch = axes.epoch.to_be_bytes();
    let mut frame = [0_u8; PUBLIC_A_TAIL_FRAME_BYTES_V2];
    let mut cursor = 0;
    for part in [
        PUBLIC_A_TAIL_DOMAIN_V2,
        version.as_slice(),
        axes.release_profile_digest.as_slice(),
        axes.target_profile_digest.as_slice(),
        axes.security_certificate_digest.as_slice(),
        axes.roster_digest.as_slice(),
        axes.key_material_digest.as_slice(),
        epoch.as_slice(),
        axes.cpk_transcript_digest.as_slice(),
        ring_degree.as_slice(),
        legacy_limbs.as_slice(),
        target_limbs.as_slice(),
        limb_bytes.as_slice(),
        modulus.as_slice(),
        root.as_slice(),
    ] {
        write_frame_part_v2(&mut frame, &mut cursor, part)?;
    }
    if cursor != frame.len() {
        return Err(RnsNativeBasisExtensionErrorV2::ResourceCeiling);
    }
    Ok(frame)
}

fn fill_public_a_tail_from_frame_v2(
    axes: &RnsNativeBasisExtensionAxesV2,
    limb: usize,
    output: &mut [u64],
) -> Result<(), RnsNativeBasisExtensionErrorV2> {
    let frame = public_a_tail_frame_v2(axes, limb)?;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    let mut stream = Shake256Reader::new(&frame);
    for coefficient in output {
        let mut accepted = None;
        for _ in 0..MAX_PUBLIC_A_CANDIDATES_PER_COEFFICIENT_V2 {
            let mut bytes = [0_u8; 8];
            stream.read(&mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < zone {
                accepted = Some(candidate % modulus);
                break;
            }
        }
        *coefficient = accepted.ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
    }
    Ok(())
}

fn derive_public_a_tail_limb_v2(
    axes: &RnsNativeBasisExtensionAxesV2,
    limb: usize,
    output: &mut [u64],
) -> Result<(), RnsNativeBasisExtensionErrorV2> {
    axes.validate_v2()?;
    if output.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 {
        return Err(RnsNativeBasisExtensionErrorV2::ResourceCeiling);
    }
    fill_public_a_tail_from_frame_v2(axes, limb, output)
}

fn try_zeroed_u64_vec_v2(length: usize) -> Result<Vec<u64>, RnsNativeBasisExtensionErrorV2> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
    output.resize(length, 0);
    if output.len() != length || output.capacity() != length {
        return Err(RnsNativeBasisExtensionErrorV2::ResourceCeiling);
    }
    Ok(output)
}

fn clear_u64_slice_v2(values: &mut [u64]) {
    let values = core::hint::black_box(values);
    values.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}

struct ZeroizingU64VecV2(Vec<u64>);

impl ZeroizingU64VecV2 {
    fn zeroed_v2(length: usize) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        Ok(Self(try_zeroed_u64_vec_v2(length)?))
    }

    fn as_slice(&self) -> &[u64] {
        self.0.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [u64] {
        self.0.as_mut_slice()
    }
}

impl Drop for ZeroizingU64VecV2 {
    fn drop(&mut self) {
        clear_u64_slice_v2(self.0.as_mut_slice());
    }
}

struct ZeroizingTwoLimbWorkspaceV2 {
    left: ZeroizingU64VecV2,
    right: ZeroizingU64VecV2,
}

impl ZeroizingTwoLimbWorkspaceV2 {
    fn new_v2() -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        Ok(Self {
            left: ZeroizingU64VecV2::zeroed_v2(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)?,
            right: ZeroizingU64VecV2::zeroed_v2(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)?,
        })
    }

    fn clear_v2(&mut self) {
        clear_u64_slice_v2(self.left.as_mut_slice());
        clear_u64_slice_v2(self.right.as_mut_slice());
    }
}

struct ZeroizingWorkspaceLeaseV2<'a> {
    workspace: &'a mut ZeroizingTwoLimbWorkspaceV2,
}

impl<'a> ZeroizingWorkspaceLeaseV2<'a> {
    fn new_v2(workspace: &'a mut ZeroizingTwoLimbWorkspaceV2) -> Self {
        workspace.clear_v2();
        Self { workspace }
    }

    fn left(&self) -> &[u64] {
        self.workspace.left.as_slice()
    }

    fn left_mut(&mut self) -> &mut [u64] {
        self.workspace.left.as_mut_slice()
    }

    fn right_mut(&mut self) -> &mut [u64] {
        self.workspace.right.as_mut_slice()
    }

    fn limbs_mut(&mut self) -> (&mut [u64], &mut [u64]) {
        (
            self.workspace.left.as_mut_slice(),
            self.workspace.right.as_mut_slice(),
        )
    }

    fn clear_v2(&mut self) {
        self.workspace.clear_v2();
    }
}

impl Drop for ZeroizingWorkspaceLeaseV2<'_> {
    fn drop(&mut self) {
        self.workspace.clear_v2();
    }
}

/// Explicit move-only owner for the C-tail arithmetic workspace.
///
/// The compiled parent allocates this owner before sampling the inherited V1
/// encryption opening, then moves that exact owner through
/// [`RnsNativeSynchronousSameOpeningBorrowV2`] into
/// [`RnsNativeCiphertextTailKernelV2`]. The live kernel constructor performs no
/// allocation. Dropping an unused owner, a rejected opening/kernel, or an
/// unwinding kernel clears both release-degree limbs through their zeroizing
/// owners. The neutral allocator alone does not attest chronology; the private
/// coordinator call site is the source-level witness for that ordering.
pub(super) struct RnsNativeCiphertextTailWorkspaceOwnerV2 {
    workspace: ZeroizingTwoLimbWorkspaceV2,
}

impl RnsNativeCiphertextTailWorkspaceOwnerV2 {
    pub(super) fn allocate_workspace_v2() -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        let owner = Self {
            workspace: ZeroizingTwoLimbWorkspaceV2::new_v2()?,
        };
        owner.validate_workspace_allocation_v2()?;
        Ok(owner)
    }

    fn validate_workspace_allocation_v2(&self) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        if self.workspace.left.as_slice().len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.workspace.left.0.capacity() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.workspace.right.as_slice().len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.workspace.right.0.capacity() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        {
            return Err(RnsNativeBasisExtensionErrorV2::ResourceCeiling);
        }
        Ok(())
    }

    fn lease_v2(&mut self) -> ZeroizingWorkspaceLeaseV2<'_> {
        ZeroizingWorkspaceLeaseV2::new_v2(&mut self.workspace)
    }
}

/// Move-only builder for the four public key-tail objects.
///
/// Each transition borrows exactly one [`ZkAmsMkheAdmittedCpkPartyV1`]. The
/// admitted wrapper can be constructed only after the V1 relation verifier and
/// ceremony admission complete. A mismatch, arithmetic error, or caught unwind
/// permanently poisons this builder; the scoped workspace lease clears both
/// secret-derived limbs on success, error, and unwind.
pub(super) struct RnsNativeCollectiveKeyTailBuilderV2 {
    axes: RnsNativeBasisExtensionAxesV2,
    public_a_tail: Vec<u64>,
    collective_b_tail: Vec<u64>,
    party_contribution_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    workspace: ZeroizingTwoLimbWorkspaceV2,
    next_party: usize,
    poisoned: bool,
}

impl RnsNativeCollectiveKeyTailBuilderV2 {
    pub(super) fn new_v2(
        axes: RnsNativeBasisExtensionAxesV2,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        axes.validate_v2()?;
        let tail_coefficients = TAIL_LIMB_COUNT_V2
            .checked_mul(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
        let mut public_a_tail = try_zeroed_u64_vec_v2(tail_coefficients)?;
        for tail in 0..TAIL_LIMB_COUNT_V2 {
            let start = tail * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
            derive_public_a_tail_limb_v2(
                &axes,
                LEGACY_LIMB_COUNT_V2 + tail,
                &mut public_a_tail[start..start + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1],
            )?;
        }
        Ok(Self {
            axes,
            public_a_tail,
            collective_b_tail: try_zeroed_u64_vec_v2(tail_coefficients)?,
            party_contribution_digests: [[0; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            workspace: ZeroizingTwoLimbWorkspaceV2::new_v2()?,
            next_party: 0,
            poisoned: false,
        })
    }

    pub(super) fn absorb_v1_admitted_party_v2(
        &mut self,
        admitted: &ZkAmsMkheAdmittedCpkPartyV1,
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.next_party >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidPartyOrder);
        }
        let state = admitted.state();
        let expected_party = self.next_party;
        if usize::from(state.party_index()) != expected_party
            || state.party() != self.axes.parties[expected_party]
            || state.profile_digest_internal() != self.axes.release_profile_digest
            || state.security_certificate_digest_internal() != self.axes.security_certificate_digest
            || state.roster_digest_internal() != self.axes.roster_digest
            || state.key_material_digest_internal() != self.axes.key_material_digest
            || state.epoch() != self.axes.epoch
            || state.transcript_digest() != self.axes.cpk_transcript_digest
            || state.public_share_digest() == [0; 32]
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAdmittedState);
        }
        let secret = state.secret().coefficients.as_slice();
        let error = state.public_error().coefficients.as_slice();
        if secret.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || error.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || !secret.iter().any(|coefficient| *coefficient != 0)
            || secret
                .iter()
                .any(|coefficient| !(-1..=1).contains(coefficient))
            || error
                .iter()
                .any(|coefficient| coefficient.unsigned_abs() > 2)
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAdmittedState);
        }

        let mut contribution_hash = Keccak256::new();
        contribution_hash.update(PARTY_B_TAIL_CONTRIBUTION_DOMAIN_V2);
        contribution_hash.update(&basis_extension_contract_digest_v2());
        contribution_hash.update(&[state.party_index()]);
        contribution_hash.update(&state.party().to_bytes());
        contribution_hash.update(&state.public_share_digest());
        let mut lease = ZeroizingWorkspaceLeaseV2::new_v2(&mut self.workspace);
        for tail in 0..TAIL_LIMB_COUNT_V2 {
            let limb = LEGACY_LIMB_COUNT_V2 + tail;
            let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
            let root = ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb];
            let start = tail * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
            let end = start + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
            lease
                .left_mut()
                .copy_from_slice(&self.public_a_tail[start..end]);
            {
                let (left, right) = lease.limbs_mut();
                dependency_v2(negacyclic_multiply_signed_rhs_two_limb_v1(
                    left, right, secret, modulus, root,
                ))?;
            }
            contribution_hash.update(&(limb as u16).to_be_bytes());
            contribution_hash.update(&modulus.to_be_bytes());
            for (aggregate, (&product, &error)) in self.collective_b_tail[start..end]
                .iter_mut()
                .zip(lease.left().iter().zip(error.iter()))
            {
                let contribution =
                    party_b_tail_contribution_coefficient_v2(product, error, modulus)?;
                *aggregate = mod_add(*aggregate, contribution, modulus);
                contribution_hash.update(&contribution.to_be_bytes());
            }
            lease.clear_v2();
        }
        self.party_contribution_digests[expected_party] = contribution_hash.finalize();
        self.next_party += 1;
        self.poisoned = false;
        Ok(())
    }

    pub(super) fn finish_v2(
        mut self,
    ) -> Result<RnsNativeCollectiveKeyTailOwnerV2, RnsNativeBasisExtensionErrorV2> {
        if self.poisoned || self.next_party != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            self.poisoned = true;
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        let integrity_digest = key_tail_integrity_digest_v2(
            &self.axes,
            &self.public_a_tail,
            &self.collective_b_tail,
            &self.party_contribution_digests,
        )?;
        if integrity_digest == [0; 32] {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
        }
        Ok(RnsNativeCollectiveKeyTailOwnerV2 {
            axes: self.axes,
            public_a_tail: self.public_a_tail,
            collective_b_tail: self.collective_b_tail,
            integrity_digest,
        })
    }
}

fn party_b_tail_contribution_coefficient_v2(
    a_times_secret: u64,
    error: i64,
    modulus: u64,
) -> Result<u64, RnsNativeBasisExtensionErrorV2> {
    if a_times_secret >= modulus || error.unsigned_abs() > 2 {
        return Err(RnsNativeBasisExtensionErrorV2::InvalidAdmittedState);
    }
    let plaintext_modulus = bytes_mod_u64(&VEGA_T256_SCALAR_MODULUS_BE_V1, modulus);
    let negated_product = if a_times_secret == 0 {
        0
    } else {
        modulus - a_times_secret
    };
    let scaled_error = mod_mul(signed_mod(error, modulus), plaintext_modulus, modulus);
    Ok(mod_add(negated_product, scaled_error, modulus))
}

fn key_tail_integrity_digest_v2(
    axes: &RnsNativeBasisExtensionAxesV2,
    public_a_tail: &[u64],
    collective_b_tail: &[u64],
    party_contribution_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], RnsNativeBasisExtensionErrorV2> {
    let expected = TAIL_LIMB_COUNT_V2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
    if public_a_tail.len() != expected
        || collective_b_tail.len() != expected
        || party_contribution_digests.contains(&[0; 32])
    {
        return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
    }
    let mut hash = Keccak256::new();
    hash.update(KEY_TAIL_INTEGRITY_DOMAIN_V2);
    hash.update(&basis_extension_contract_digest_v2());
    hash.update(&axes.release_profile_digest);
    hash.update(&axes.target_profile_digest);
    hash.update(&axes.security_certificate_digest);
    hash.update(&axes.roster_digest);
    hash.update(&axes.key_material_digest);
    hash.update(&axes.epoch.to_be_bytes());
    hash.update(&axes.cpk_transcript_digest);
    for (role, coefficients) in [public_a_tail, collective_b_tail].into_iter().enumerate() {
        hash.update(&[role as u8]);
        for tail in 0..TAIL_LIMB_COUNT_V2 {
            let limb = LEGACY_LIMB_COUNT_V2 + tail;
            let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
            let start = tail * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
            let end = start + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
            hash.update(&(limb as u16).to_be_bytes());
            hash.update(&modulus.to_be_bytes());
            for coefficient in &coefficients[start..end] {
                if *coefficient >= modulus {
                    return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
                }
                hash.update(&coefficient.to_be_bytes());
            }
        }
    }
    for digest in party_contribution_digests {
        hash.update(digest);
    }
    Ok(hash.finalize())
}

/// Move-only public key-tail owner. Its digest is an internal integrity check,
/// not a publication receipt, provider snapshot, terminal, or release token.
pub(super) struct RnsNativeCollectiveKeyTailOwnerV2 {
    axes: RnsNativeBasisExtensionAxesV2,
    public_a_tail: Vec<u64>,
    collective_b_tail: Vec<u64>,
    integrity_digest: [u8; 32],
}

impl RnsNativeCollectiveKeyTailOwnerV2 {
    #[cfg(test)]
    pub(super) fn malformed_test_owner_v2() -> Self {
        Self {
            axes: RnsNativeBasisExtensionAxesV2 {
                release_profile_digest: [0x11; 32],
                target_profile_digest: [0x22; 32],
                security_certificate_digest: [0x33; 32],
                roster_digest: [0x44; 32],
                key_material_digest: [0x55; 32],
                epoch: 1,
                cpk_transcript_digest: [0x66; 32],
                parties: core::array::from_fn(|index| {
                    ZkAmsMkhePartyIdV1::new([index as u8 + 1; 32]).expect("nonzero test-only party")
                }),
            },
            public_a_tail: Vec::new(),
            collective_b_tail: Vec::new(),
            integrity_digest: [0x77; 32],
        }
    }

    /// Reject a foreign V1 authority before the sole key-tail publication
    /// visit. Keeping this check on the unconsumed owner prevents even an
    /// unauthorizing CAS orphan for mismatched governed axes.
    pub(super) fn validate_v1_authority_binding_v2(
        &self,
        authority: &ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        self.axes.validate_v2()?;
        if authority.profile_digest() != self.axes.release_profile_digest
            || authority.security_certificate_digest() != self.axes.security_certificate_digest
            || authority.roster_digest() != self.axes.roster_digest
            || authority.key_material_digest() != self.axes.key_material_digest
            || authority.epoch() != self.axes.epoch
            || authority.transcript_digest() != self.axes.cpk_transcript_digest
            || authority.authority_digest() == [0; 32]
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidAxes);
        }
        Ok(())
    }

    fn limb_v2(&self, role: RnsNativeTailObjectRoleV2, limb: usize) -> Option<&[u64]> {
        if !(LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2).contains(&limb) {
            return None;
        }
        let start = (limb - LEGACY_LIMB_COUNT_V2) * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
        let end = start + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
        match role {
            RnsNativeTailObjectRoleV2::PublicA => self.public_a_tail.get(start..end),
            RnsNativeTailObjectRoleV2::CollectivePublicB => self.collective_b_tail.get(start..end),
            RnsNativeTailObjectRoleV2::CiphertextC0 | RnsNativeTailObjectRoleV2::CiphertextC1 => {
                None
            }
        }
    }

    fn visit_key_tail_coefficients_v2<V: RnsNativeTailCoefficientVisitorV2 + ?Sized>(
        &self,
        visitor: &mut V,
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        for role in [
            RnsNativeTailObjectRoleV2::PublicA,
            RnsNativeTailObjectRoleV2::CollectivePublicB,
        ] {
            for limb in LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2 {
                let position = RnsNativeTailSourcePositionV2::new_key_v2(role, limb)?;
                visitor.visit_tail_coefficients_v2(
                    position,
                    self.limb_v2(role, limb)
                        .ok_or(RnsNativeBasisExtensionErrorV2::InvalidPosition)?,
                )?;
            }
        }
        Ok(())
    }

    /// Establish the allocation-free 43-record checksum chronology before
    /// the sole key-tail CAS visit can begin.
    pub(super) fn begin_ciphertext_tail_lifecycle_v2(
        &self,
    ) -> Result<RnsNativeCiphertextTailLifecycleV2, RnsNativeBasisExtensionErrorV2> {
        RnsNativeCiphertextTailLifecycleV2::new_v2(self)
    }

    /// Consume the sole arithmetic owner into the publication boundary.
    ///
    /// The returned wrapper exposes no coefficient visitor, so neither a
    /// successful publication nor a consuming failure can replay the four key
    /// tail objects from the same owner. The wrapper retains the arithmetic
    /// owner only because the 43 same-opening ciphertext tails still need to
    /// borrow it.
    pub(super) fn publish_key_tail_once_v2<V: RnsNativeTailCoefficientVisitorV2 + ?Sized>(
        self,
        visitor: &mut V,
    ) -> Result<RnsNativePublishedCollectiveKeyTailOwnerV2, RnsNativeBasisExtensionErrorV2> {
        self.visit_key_tail_coefficients_v2(visitor)?;
        Ok(RnsNativePublishedCollectiveKeyTailOwnerV2 { key_tail: self })
    }
}

/// Move-only key-tail arithmetic owner after its sole publication visit.
///
/// No method exposes the retained coefficient vectors or permits a second key
/// publication pass. All remaining operations are exact forward transitions
/// needed by the record-local same-opening lifecycle.
pub(super) struct RnsNativePublishedCollectiveKeyTailOwnerV2 {
    key_tail: RnsNativeCollectiveKeyTailOwnerV2,
}

impl RnsNativePublishedCollectiveKeyTailOwnerV2 {
    pub(super) const fn integrity_digest_v2(&self) -> [u8; 32] {
        self.key_tail.integrity_digest
    }

    /// Bind the retained arithmetic owner to the exact finalized V1 key
    /// authority before any prefix or tail descriptor may enter a manifest.
    /// This compares the governed axes rather than accepting copied digests
    /// from a caller.
    pub(super) fn validate_v1_authority_binding_v2(
        &self,
        authority: &ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        self.key_tail.validate_v1_authority_binding_v2(authority)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind_v1_synchronous_callback_v2<'opening>(
        &self,
        workspace: RnsNativeCiphertextTailWorkspaceOwnerV2,
        record_ordinal: u8,
        sample_index: u64,
        canonical_plaintext: &'opening [[u8; 32]],
        ephemeral: &'opening [i64],
        error_zero: &'opening [i64],
        error_one: &'opening [i64],
        encryption_nonce: &'opening [u8; 32],
    ) -> Result<RnsNativeSynchronousSameOpeningBorrowV2<'opening>, RnsNativeBasisExtensionErrorV2>
    {
        RnsNativeSynchronousSameOpeningBorrowV2::from_v1_synchronous_callback_v2(
            &self.key_tail,
            workspace,
            record_ordinal,
            sample_index,
            canonical_plaintext,
            ephemeral,
            error_zero,
            error_one,
            encryption_nonce,
        )
    }

    pub(super) fn emit_ciphertext_tail_once_v2<V: RnsNativeTailCoefficientVisitorV2 + ?Sized>(
        &self,
        opening: RnsNativeSynchronousSameOpeningBorrowV2<'_>,
        visitor: &mut V,
    ) -> Result<RnsNativeCiphertextTailCompletionV2, RnsNativeBasisExtensionErrorV2> {
        RnsNativeCiphertextTailKernelV2::new_v2(&self.key_tail, opening)?.emit_v2(visitor)
    }
}

/// Sole logical role order: `A`, `B`, every record's `C0`, every record's
/// `C1`. Physical record-local computation may occur earlier, but it cannot
/// change these source ordinals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum RnsNativeTailObjectRoleV2 {
    PublicA = 0,
    CollectivePublicB = 1,
    CiphertextC0 = 2,
    CiphertextC1 = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeTailSourcePositionV2 {
    role: RnsNativeTailObjectRoleV2,
    record_ordinal: Option<u8>,
    limb: u8,
}

impl RnsNativeTailSourcePositionV2 {
    fn new_key_v2(
        role: RnsNativeTailObjectRoleV2,
        limb: usize,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        if !matches!(
            role,
            RnsNativeTailObjectRoleV2::PublicA | RnsNativeTailObjectRoleV2::CollectivePublicB
        ) || !(LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2).contains(&limb)
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition);
        }
        Ok(Self {
            role,
            record_ordinal: None,
            limb: limb as u8,
        })
    }

    fn new_ciphertext_v2(
        role: RnsNativeTailObjectRoleV2,
        record_ordinal: u8,
        limb: usize,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        if !matches!(
            role,
            RnsNativeTailObjectRoleV2::CiphertextC0 | RnsNativeTailObjectRoleV2::CiphertextC1
        ) || usize::from(record_ordinal) >= RECORD_COUNT_V2
            || !(LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2).contains(&limb)
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition);
        }
        Ok(Self {
            role,
            record_ordinal: Some(record_ordinal),
            limb: limb as u8,
        })
    }

    pub(super) fn from_tail_ordinal_v2(
        tail_ordinal: usize,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        if tail_ordinal >= MISSING_TAIL_OBJECT_COUNT_V2 {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition);
        }
        if tail_ordinal < TAIL_LIMB_COUNT_V2 {
            return Self::new_key_v2(
                RnsNativeTailObjectRoleV2::PublicA,
                LEGACY_LIMB_COUNT_V2 + tail_ordinal,
            );
        }
        if tail_ordinal < KEY_TAIL_OBJECT_COUNT_V2 {
            return Self::new_key_v2(
                RnsNativeTailObjectRoleV2::CollectivePublicB,
                LEGACY_LIMB_COUNT_V2 + tail_ordinal - TAIL_LIMB_COUNT_V2,
            );
        }
        let ciphertext_ordinal = tail_ordinal - KEY_TAIL_OBJECT_COUNT_V2;
        let component_span = RECORD_COUNT_V2 * TAIL_LIMB_COUNT_V2;
        let (role, within_component) = if ciphertext_ordinal < component_span {
            (RnsNativeTailObjectRoleV2::CiphertextC0, ciphertext_ordinal)
        } else {
            (
                RnsNativeTailObjectRoleV2::CiphertextC1,
                ciphertext_ordinal - component_span,
            )
        };
        Self::new_ciphertext_v2(
            role,
            u8::try_from(within_component / TAIL_LIMB_COUNT_V2)
                .map_err(|_| RnsNativeBasisExtensionErrorV2::InvalidPosition)?,
            LEGACY_LIMB_COUNT_V2 + within_component % TAIL_LIMB_COUNT_V2,
        )
    }

    pub(super) const fn role_v2(self) -> RnsNativeTailObjectRoleV2 {
        self.role
    }

    pub(super) const fn record_ordinal_v2(self) -> Option<u8> {
        self.record_ordinal
    }

    pub(super) const fn limb_v2(self) -> u8 {
        self.limb
    }

    pub(super) const fn object_kind_v2(self) -> ZkAmsMkheDirectObjectKindV1 {
        match self.role {
            RnsNativeTailObjectRoleV2::PublicA => ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            RnsNativeTailObjectRoleV2::CollectivePublicB => {
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB
            }
            RnsNativeTailObjectRoleV2::CiphertextC0 => {
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0
            }
            RnsNativeTailObjectRoleV2::CiphertextC1 => {
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1
            }
        }
    }

    pub(super) fn tail_ordinal_v2(self) -> Result<usize, RnsNativeBasisExtensionErrorV2> {
        let tail = usize::from(self.limb)
            .checked_sub(LEGACY_LIMB_COUNT_V2)
            .filter(|tail| *tail < TAIL_LIMB_COUNT_V2)
            .ok_or(RnsNativeBasisExtensionErrorV2::InvalidPosition)?;
        match (self.role, self.record_ordinal) {
            (RnsNativeTailObjectRoleV2::PublicA, None) => Ok(tail),
            (RnsNativeTailObjectRoleV2::CollectivePublicB, None) => Ok(TAIL_LIMB_COUNT_V2 + tail),
            (RnsNativeTailObjectRoleV2::CiphertextC0, Some(record))
                if usize::from(record) < RECORD_COUNT_V2 =>
            {
                Ok(KEY_TAIL_OBJECT_COUNT_V2 + usize::from(record) * TAIL_LIMB_COUNT_V2 + tail)
            }
            (RnsNativeTailObjectRoleV2::CiphertextC1, Some(record))
                if usize::from(record) < RECORD_COUNT_V2 =>
            {
                Ok(KEY_TAIL_OBJECT_COUNT_V2
                    + RECORD_COUNT_V2 * TAIL_LIMB_COUNT_V2
                    + usize::from(record) * TAIL_LIMB_COUNT_V2
                    + tail)
            }
            _ => Err(RnsNativeBasisExtensionErrorV2::InvalidPosition),
        }
    }

    pub(super) fn full_source_ordinal_v2(self) -> Result<usize, RnsNativeBasisExtensionErrorV2> {
        let limb = usize::from(self.limb);
        if !(LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2).contains(&limb) {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition);
        }
        match (self.role, self.record_ordinal) {
            (RnsNativeTailObjectRoleV2::PublicA, None) => Ok(limb),
            (RnsNativeTailObjectRoleV2::CollectivePublicB, None) => Ok(TARGET_LIMB_COUNT_V2 + limb),
            (RnsNativeTailObjectRoleV2::CiphertextC0, Some(record))
                if usize::from(record) < RECORD_COUNT_V2 =>
            {
                Ok(2 * TARGET_LIMB_COUNT_V2 + usize::from(record) * TARGET_LIMB_COUNT_V2 + limb)
            }
            (RnsNativeTailObjectRoleV2::CiphertextC1, Some(record))
                if usize::from(record) < RECORD_COUNT_V2 =>
            {
                Ok(2 * TARGET_LIMB_COUNT_V2
                    + RECORD_COUNT_V2 * TARGET_LIMB_COUNT_V2
                    + usize::from(record) * TARGET_LIMB_COUNT_V2
                    + limb)
            }
            _ => Err(RnsNativeBasisExtensionErrorV2::InvalidPosition),
        }
    }

    fn modulus_v2(self) -> Result<u64, RnsNativeBasisExtensionErrorV2> {
        self.tail_ordinal_v2()?;
        Ok(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[usize::from(self.limb)])
    }
}

/// Synchronous coefficient sink only. Implementing this trait cannot mint a
/// CAS receipt, provider snapshot, manifest, terminal, or release capability.
pub(super) trait RnsNativeTailCoefficientVisitorV2 {
    fn visit_tail_coefficients_v2(
        &mut self,
        position: RnsNativeTailSourcePositionV2,
        coefficients: &[u64],
    ) -> Result<(), RnsNativeBasisExtensionErrorV2>;
}

/// Stateful canonical encoder for exactly `u32(N) || N*u64`, all big-endian.
/// The API supplies no chunk index: after the one count prefix, it can emit
/// only chunks `0..128` in order, and `finish_v2` consumes and rejects an
/// incomplete encoder. Its digest covers raw bytes only and is not a receipt.
pub(super) struct RnsNativeTailObjectEncoderV2<'a> {
    position: RnsNativeTailSourcePositionV2,
    coefficients: &'a [u64],
    hash: Keccak256,
    prefix_written: bool,
    next_chunk: usize,
    poisoned: bool,
}

pub(super) struct RnsNativeEncodedTailObjectV2 {
    pub(super) position: RnsNativeTailSourcePositionV2,
    pub(super) encoded_bytes: u32,
    pub(super) encoded_bytes_digest: [u8; 32],
}

impl<'a> RnsNativeTailObjectEncoderV2<'a> {
    pub(super) fn new_v2(
        position: RnsNativeTailSourcePositionV2,
        coefficients: &'a [u64],
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        let modulus = position.modulus_v2()?;
        if coefficients.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || coefficients
                .iter()
                .any(|coefficient| *coefficient >= modulus)
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
        }
        Ok(Self {
            position,
            coefficients,
            hash: Keccak256::new(),
            prefix_written: false,
            next_chunk: 0,
            poisoned: false,
        })
    }

    pub(super) fn write_count_prefix_v2(
        &mut self,
        output: &mut [u8; COUNT_PREFIX_BYTES_V2],
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        if self.prefix_written || self.next_chunk != 0 {
            return Err(RnsNativeBasisExtensionErrorV2::IncompleteObject);
        }
        *output = u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?
            .to_be_bytes();
        self.hash.update(output);
        self.prefix_written = true;
        self.poisoned = false;
        Ok(())
    }

    pub(super) fn write_next_chunk_v2(
        &mut self,
        output: &mut [u8; CHUNK_BYTES_V2],
    ) -> Result<u16, RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        if !self.prefix_written || self.next_chunk >= CHUNK_COUNT_PER_OBJECT_V2 {
            return Err(RnsNativeBasisExtensionErrorV2::IncompleteObject);
        }
        let start = self.next_chunk * COEFFICIENTS_PER_CHUNK_V2;
        let end = start + COEFFICIENTS_PER_CHUNK_V2;
        for (encoded, coefficient) in output
            .chunks_exact_mut(core::mem::size_of::<u64>())
            .zip(&self.coefficients[start..end])
        {
            encoded.copy_from_slice(&coefficient.to_be_bytes());
        }
        self.hash.update(output);
        let emitted = u16::try_from(self.next_chunk)
            .map_err(|_| RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
        self.next_chunk += 1;
        self.poisoned = false;
        Ok(emitted)
    }

    pub(super) fn finish_v2(
        mut self,
    ) -> Result<RnsNativeEncodedTailObjectV2, RnsNativeBasisExtensionErrorV2> {
        if self.poisoned || !self.prefix_written || self.next_chunk != CHUNK_COUNT_PER_OBJECT_V2 {
            self.poisoned = true;
            return Err(RnsNativeBasisExtensionErrorV2::IncompleteObject);
        }
        self.poisoned = true;
        Ok(RnsNativeEncodedTailObjectV2 {
            position: self.position,
            encoded_bytes: OBJECT_BYTES_V2 as u32,
            encoded_bytes_digest: self.hash.finalize(),
        })
    }
}

/// Borrow of the exact live V1 same-opening values. Only the parent
/// `incremental_source` module may call the constructor. It must also supply
/// an exact allocated workspace owner and moves it onward. The lifetime bars
/// retaining any witness after the callback returns, but this type alone does
/// not prove that the absent parent call site allocated the owner before
/// entropy.
pub(super) struct RnsNativeSynchronousSameOpeningBorrowV2<'opening> {
    workspace: RnsNativeCiphertextTailWorkspaceOwnerV2,
    key_tail_integrity_digest: [u8; 32],
    record_ordinal: u8,
    sample_index: u64,
    canonical_plaintext: &'opening [[u8; 32]],
    ephemeral: &'opening [i64],
    error_zero: &'opening [i64],
    error_one: &'opening [i64],
    encryption_nonce: &'opening [u8; 32],
}

impl<'opening> RnsNativeSynchronousSameOpeningBorrowV2<'opening> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_v1_synchronous_callback_v2(
        key_tail: &RnsNativeCollectiveKeyTailOwnerV2,
        workspace: RnsNativeCiphertextTailWorkspaceOwnerV2,
        record_ordinal: u8,
        sample_index: u64,
        canonical_plaintext: &'opening [[u8; 32]],
        ephemeral: &'opening [i64],
        error_zero: &'opening [i64],
        error_one: &'opening [i64],
        encryption_nonce: &'opening [u8; 32],
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        let opening = Self {
            workspace,
            key_tail_integrity_digest: key_tail.integrity_digest,
            record_ordinal,
            sample_index,
            canonical_plaintext,
            ephemeral,
            error_zero,
            error_one,
            encryption_nonce,
        };
        opening.validate_v2()?;
        Ok(opening)
    }

    fn validate_v2(&self) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        self.workspace.validate_workspace_allocation_v2()?;
        if self.key_tail_integrity_digest == [0; 32]
            || usize::from(self.record_ordinal) >= RECORD_COUNT_V2
            || self.sample_index != u64::from(self.record_ordinal)
            || *self.encryption_nonce == [0; 32]
            || self.canonical_plaintext.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.ephemeral.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.error_zero.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.error_one.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self
                .canonical_plaintext
                .iter()
                .any(|coefficient| *coefficient >= VEGA_T256_SCALAR_MODULUS_BE_V1)
            || !self.ephemeral.iter().any(|coefficient| *coefficient != 0)
            || self
                .ephemeral
                .iter()
                .any(|coefficient| !(-1..=1).contains(coefficient))
            || self
                .error_zero
                .iter()
                .chain(self.error_one.iter())
                .any(|coefficient| coefficient.unsigned_abs() > 2)
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidSameOpening);
        }
        Ok(())
    }
}

/// Move-only, one-record C-tail arithmetic kernel. It emits four synchronous
/// borrows in physical generation order `C0[38], C0[39], C1[38], C1[39]`.
/// Positions retain the distinct canonical source ordinals, so a future
/// manifest can order all 43 C0 records before all 43 C1 records. No such
/// manifest or publication transaction is implemented here.
pub(super) struct RnsNativeCiphertextTailKernelV2<'key, 'opening> {
    key_tail: &'key RnsNativeCollectiveKeyTailOwnerV2,
    opening: RnsNativeSynchronousSameOpeningBorrowV2<'opening>,
    poisoned: bool,
}

/// Non-authorizing record-local checksum. This is not a terminal capability.
pub(super) struct RnsNativeCiphertextTailCompletionV2 {
    key_tail_integrity_digest: [u8; 32],
    record_ordinal: u8,
    sample_index: u64,
    emitted_limb_count: u8,
    coefficient_digest: [u8; 32],
}

impl RnsNativeCiphertextTailCompletionV2 {
    pub(super) const fn record_ordinal_v2(&self) -> u8 {
        self.record_ordinal
    }

    pub(super) const fn sample_index_v2(&self) -> u64 {
        self.sample_index
    }

    pub(super) const fn emitted_limb_count_v2(&self) -> u8 {
        self.emitted_limb_count
    }

    pub(super) const fn coefficient_digest_v2(&self) -> [u8; 32] {
        self.coefficient_digest
    }
}

impl<'key, 'opening> RnsNativeCiphertextTailKernelV2<'key, 'opening> {
    pub(super) fn new_v2(
        key_tail: &'key RnsNativeCollectiveKeyTailOwnerV2,
        opening: RnsNativeSynchronousSameOpeningBorrowV2<'opening>,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        opening.validate_v2()?;
        key_tail.axes.validate_v2()?;
        if opening.key_tail_integrity_digest != key_tail.integrity_digest {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidSameOpening);
        }
        Ok(Self {
            key_tail,
            opening,
            poisoned: false,
        })
    }

    pub(super) fn emit_v2<V: RnsNativeTailCoefficientVisitorV2 + ?Sized>(
        mut self,
        visitor: &mut V,
    ) -> Result<RnsNativeCiphertextTailCompletionV2, RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        let mut hash = Keccak256::new();
        hash.update(CIPHERTEXT_TAIL_COMPLETION_DOMAIN_V2);
        hash.update(&basis_extension_contract_digest_v2());
        hash.update(&self.key_tail.integrity_digest);
        hash.update(&[self.opening.record_ordinal]);
        hash.update(&self.opening.sample_index.to_be_bytes());
        hash.update(self.opening.encryption_nonce);
        let mut emitted_limb_count = 0_u8;
        let mut lease = self.opening.workspace.lease_v2();
        for role in [
            RnsNativeTailObjectRoleV2::CiphertextC0,
            RnsNativeTailObjectRoleV2::CiphertextC1,
        ] {
            for limb in LEGACY_LIMB_COUNT_V2..TARGET_LIMB_COUNT_V2 {
                let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
                let root = ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb];
                let source_role = match role {
                    RnsNativeTailObjectRoleV2::CiphertextC0 => {
                        RnsNativeTailObjectRoleV2::CollectivePublicB
                    }
                    RnsNativeTailObjectRoleV2::CiphertextC1 => RnsNativeTailObjectRoleV2::PublicA,
                    _ => return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition),
                };
                lease.left_mut().copy_from_slice(
                    self.key_tail
                        .limb_v2(source_role, limb)
                        .ok_or(RnsNativeBasisExtensionErrorV2::InvalidPosition)?,
                );
                {
                    let (left, right) = lease.limbs_mut();
                    dependency_v2(negacyclic_multiply_signed_rhs_two_limb_v1(
                        left,
                        right,
                        self.opening.ephemeral,
                        modulus,
                        root,
                    ))?;
                }
                let error = match role {
                    RnsNativeTailObjectRoleV2::CiphertextC0 => self.opening.error_zero,
                    RnsNativeTailObjectRoleV2::CiphertextC1 => self.opening.error_one,
                    _ => return Err(RnsNativeBasisExtensionErrorV2::InvalidPosition),
                };
                add_error_and_optional_message_v2(
                    lease.left_mut(),
                    error,
                    if role == RnsNativeTailObjectRoleV2::CiphertextC0 {
                        Some(self.opening.canonical_plaintext)
                    } else {
                        None
                    },
                    modulus,
                )?;
                let position = RnsNativeTailSourcePositionV2::new_ciphertext_v2(
                    role,
                    self.opening.record_ordinal,
                    limb,
                )?;
                hash.update(&(position.tail_ordinal_v2()? as u16).to_be_bytes());
                hash.update(&modulus.to_be_bytes());
                for coefficient in lease.left() {
                    hash.update(&coefficient.to_be_bytes());
                }
                visitor
                    .visit_tail_coefficients_v2(position, lease.left())
                    .map_err(|_| RnsNativeBasisExtensionErrorV2::VisitorRejected)?;
                emitted_limb_count = emitted_limb_count
                    .checked_add(1)
                    .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
                lease.clear_v2();
            }
        }
        drop(lease);
        let coefficient_digest = hash.finalize();
        if emitted_limb_count != CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2
            || coefficient_digest == [0; 32]
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
        }
        self.poisoned = false;
        Ok(RnsNativeCiphertextTailCompletionV2 {
            key_tail_integrity_digest: self.key_tail.integrity_digest,
            record_ordinal: self.opening.record_ordinal,
            sample_index: self.opening.sample_index,
            emitted_limb_count,
            coefficient_digest,
        })
    }
}

/// Move-only, non-authorizing coordinator for the exact 43-record C-tail run.
///
/// Record-local completions are consumed strictly in `0..43` order. Any
/// duplicate, hole, mismatch, error, or unwind after a transition begins
/// leaves this coordinator permanently poisoned. The final value binds all 43
/// local completion digests but is only an integrity checksum: it is not a CAS
/// receipt, provider snapshot, terminal, adapter, or release authority.
pub(super) struct RnsNativeCiphertextTailLifecycleV2 {
    key_tail_integrity_digest: [u8; 32],
    completion_hash: Keccak256,
    next_record_ordinal: u8,
    emitted_limb_count: u16,
    poisoned: bool,
}

/// Aggregate integrity checksum only. It grants no publication or release
/// authority and deliberately carries no V1 receipt or terminal state.
pub(super) struct RnsNativeCiphertextTailAggregateChecksumV2 {
    pub(super) record_count: u8,
    pub(super) emitted_limb_count: u16,
    pub(super) completion_digest: [u8; 32],
}

impl RnsNativeCiphertextTailAggregateChecksumV2 {
    pub(super) const fn completion_digest_v2(&self) -> [u8; 32] {
        self.completion_digest
    }
}

impl RnsNativeCiphertextTailLifecycleV2 {
    pub(super) fn new_v2(
        key_tail: &RnsNativeCollectiveKeyTailOwnerV2,
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        key_tail.axes.validate_v2()?;
        Self::from_key_tail_integrity_digest_v2(key_tail.integrity_digest)
    }

    fn from_key_tail_integrity_digest_v2(
        key_tail_integrity_digest: [u8; 32],
    ) -> Result<Self, RnsNativeBasisExtensionErrorV2> {
        if key_tail_integrity_digest == [0; 32] {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidRecordCompletion);
        }
        let mut completion_hash = Keccak256::new();
        completion_hash.update(CIPHERTEXT_TAIL_LIFECYCLE_DOMAIN_V2);
        completion_hash.update(&basis_extension_contract_digest_v2());
        completion_hash.update(&key_tail_integrity_digest);
        completion_hash.update(&(RECORD_COUNT_V2 as u16).to_be_bytes());
        completion_hash.update(&[CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2]);
        Ok(Self {
            key_tail_integrity_digest,
            completion_hash,
            next_record_ordinal: 0,
            emitted_limb_count: 0,
            poisoned: false,
        })
    }

    fn begin_transition_v2(&mut self) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        Ok(())
    }

    pub(super) fn accept_record_completion_v2(
        &mut self,
        completion: RnsNativeCiphertextTailCompletionV2,
    ) -> Result<(), RnsNativeBasisExtensionErrorV2> {
        self.begin_transition_v2()?;
        if usize::from(self.next_record_ordinal) >= RECORD_COUNT_V2
            || completion.record_ordinal != self.next_record_ordinal
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidRecordOrder);
        }
        if completion.key_tail_integrity_digest != self.key_tail_integrity_digest
            || completion.sample_index != u64::from(completion.record_ordinal)
            || completion.emitted_limb_count != CIPHERTEXT_TAIL_LIMBS_PER_RECORD_V2
            || completion.coefficient_digest == [0; 32]
        {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidRecordCompletion);
        }
        self.completion_hash.update(&[completion.record_ordinal]);
        self.completion_hash
            .update(&completion.sample_index.to_be_bytes());
        self.completion_hash
            .update(&[completion.emitted_limb_count]);
        self.completion_hash.update(&completion.coefficient_digest);
        self.next_record_ordinal = self
            .next_record_ordinal
            .checked_add(1)
            .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
        self.emitted_limb_count = self
            .emitted_limb_count
            .checked_add(u16::from(completion.emitted_limb_count))
            .ok_or(RnsNativeBasisExtensionErrorV2::ResourceCeiling)?;
        self.poisoned = false;
        Ok(())
    }

    pub(super) fn finish_v2(
        mut self,
    ) -> Result<RnsNativeCiphertextTailAggregateChecksumV2, RnsNativeBasisExtensionErrorV2> {
        if self.poisoned {
            return Err(RnsNativeBasisExtensionErrorV2::Poisoned);
        }
        self.poisoned = true;
        if usize::from(self.next_record_ordinal) != RECORD_COUNT_V2
            || usize::from(self.emitted_limb_count) != CIPHERTEXT_TAIL_OBJECT_COUNT_V2
        {
            return Err(RnsNativeBasisExtensionErrorV2::IncompleteLifecycle);
        }
        let completion_digest = self.completion_hash.finalize();
        if completion_digest == [0; 32] {
            return Err(RnsNativeBasisExtensionErrorV2::InvalidRecordCompletion);
        }
        Ok(RnsNativeCiphertextTailAggregateChecksumV2 {
            record_count: self.next_record_ordinal,
            emitted_limb_count: self.emitted_limb_count,
            completion_digest,
        })
    }
}

fn add_error_and_optional_message_v2(
    product: &mut [u64],
    error: &[i64],
    canonical_plaintext: Option<&[[u8; 32]]>,
    modulus: u64,
) -> Result<(), RnsNativeBasisExtensionErrorV2> {
    if product.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        || error.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        || canonical_plaintext
            .is_some_and(|plaintext| plaintext.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        || product.iter().any(|coefficient| *coefficient >= modulus)
    {
        return Err(RnsNativeBasisExtensionErrorV2::InvalidResidue);
    }
    let plaintext_modulus = bytes_mod_u64(&VEGA_T256_SCALAR_MODULUS_BE_V1, modulus);
    for index in 0..product.len() {
        let scaled_error = mod_mul(
            signed_mod(error[index], modulus),
            plaintext_modulus,
            modulus,
        );
        product[index] = mod_add(product[index], scaled_error, modulus);
        if let Some(plaintext) = canonical_plaintext {
            if plaintext[index] >= VEGA_T256_SCALAR_MODULUS_BE_V1 {
                return Err(RnsNativeBasisExtensionErrorV2::InvalidSameOpening);
            }
            product[index] = mod_add(
                product[index],
                t256_centered_residue_with_modulus_residue(
                    &plaintext[index],
                    modulus,
                    plaintext_modulus,
                ),
                modulus,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "incremental_source_rns_native_basis_extension_v2_tests.rs"]
mod tests;
