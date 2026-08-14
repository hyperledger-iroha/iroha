//! Private Phase-23 source-algebra prerequisite.
//!
//! This child freezes the exact ciphertext/source coordinate map and the
//! future aggregation transcript, but deliberately constructs no relation
//! polynomial and mints no proof, writer, snapshot, or qPCS authority. The
//! ordering seal at its parent seam is uninhabited in production; the separate
//! radix/Hyrax seal is accepted only after authenticated source replay exists.
//! Test-only permits exercise framing without making the existing Phase-23
//! context seal or any release capability constructible.

#![allow(
    dead_code,
    reason = "both production seals are intentionally uninhabited"
)]
use super::super::super::super::{
    ZkAmsMkheErrorV1,
    direct_object_transport::{
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectReadReceiptV1,
    },
    manifest::RELEASE_MODULI_V1,
};
use super::super::streaming_source_snapshot_axes_v1;
use super::{
    PHASE23_MANIFEST_CAPACITY_V1, PHASE23_RECORD_COUNT_V1, PHASE23_RING_DEGREE_V1,
    ZkAmsPhase23MaterializedEncryptedSourceOwnerV1, phase23_record_position_v1,
};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};
use core::convert::Infallible;
const SOURCE_ALGEBRA_VERSION_V2: u8 = 2;
const SOURCE_ALGEBRA_RECORDS_V2: usize = 43;
const SOURCE_ALGEBRA_EQUATIONS_V2: usize = 2;
const SOURCE_ALGEBRA_LIMBS_V2: usize = 38;
const SOURCE_ALGEBRA_RELATION_COORDINATES_V2: usize =
    SOURCE_ALGEBRA_RECORDS_V2 * SOURCE_ALGEBRA_EQUATIONS_V2 * SOURCE_ALGEBRA_LIMBS_V2;
const SOURCE_ALGEBRA_AGGREGATE_REPETITIONS_V2: usize = 5;
const SOURCE_ALGEBRA_CHALLENGE_PAIRS_V2: usize =
    SOURCE_ALGEBRA_LIMBS_V2 * SOURCE_ALGEBRA_AGGREGATE_REPETITIONS_V2;
const SOURCE_ALGEBRA_PRODUCT_COEFFICIENTS_V2: usize = 2 * PHASE23_RING_DEGREE_V1;
const SOURCE_ALGEBRA_P_COEFFICIENTS_V2: usize = 2 * PHASE23_RING_DEGREE_V1;
const SOURCE_ALGEBRA_H_COEFFICIENTS_V2: usize = PHASE23_RING_DEGREE_V1;
const SOURCE_ALGEBRA_FORMULA_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.source-algebra.formula";
const SOURCE_ALGEBRA_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.record-equation-limb-map";
const SOURCE_ALGEBRA_ORDERED_BUNDLE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.ordered-ciphertext-bundle";
const SOURCE_ALGEBRA_SOURCE_LINEAGE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.ciphertext-source-lineage";
const SOURCE_ALGEBRA_OUTPUT_LINEAGE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.ciphertext-output-lineage";
const SOURCE_ALGEBRA_PREFLIGHT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.preflight";
const SOURCE_ALGEBRA_AGGREGATE_SCHEDULE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.gamma-beta-schedule";
const SOURCE_ALGEBRA_PREREQUISITE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.source-algebra.prerequisite";
const ORDINARY_PRODUCT_FORMULA_V2: &[u8] = b"T[j,e,l]=ordinary(K[e,l]*r[j,l]);len(T)=2N";
const QUOTIENT_FORMULA_V2: &[u8] = b"H[j,e,l][i]=T[j,e,l][N+i];H[N-1]=0";
const RELATION_FORMULA_V2: &[u8] = b"P=T+p_l*E+delta*M-C=(X^N+1)*H mod q_l";
const TOP_ZERO_FORMULA_V2: &[u8] = b"P[2N-1]=H[N-1]=0";
const LIMB_FORMULA_V2: &[u8] = b"p_l=canonical_T256_mod_q_l";
const CENTERING_FORMULA_V2: &[u8] = b"M_l=m_if_m<=(p-1)/2_else_m-p;then_canonical_mod_q_l";
const EQUATION_ZERO_FORMULA_V2: &[u8] = b"e=0:K=B,E=e0,C=C0,delta=1";
const EQUATION_ONE_FORMULA_V2: &[u8] = b"e=1:K=A,E=e1,C=C1,delta=0";
const AGGREGATE_FORMULA_V2: &[u8] = b"sum_j gamma_lk^j*(equation0+beta_lk*equation1)";
const AGGREGATE_EQUIVALENT_FORMULA_V2: &[u8] =
    b"R=sum_j gamma^j*r_j;K=B+beta*A;E=sum_j gamma^j*(e0_j+beta*e1_j);M=sum_j gamma^j*M_j;C=sum_j gamma^j*(C0_j+beta*C1_j);T=ordinary(K*R)";
const AGGREGATE_EMISSION_ORDER_V2: &[u8] = b"limb->repetition->block->P-low->P-high->H";
const COMMITMENT_CHALLENGE_ORDER_V2: &[u8] =
    b"context/formula/map->all-future-relation-commitments->gamma->beta";
const CHALLENGE_RULE_V2: &[u8] =
    b"gamma_lk,beta_lk:unbiased-nonzero-and-distinct-with-domain-separated-rejection";
const SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V2: bool = false;
const SOURCE_ALGEBRA_VERIFIED_V2: bool = false;
const RADIX_PACKING_VERIFIED_V2: bool = false;
const RADIX_CARRY_VERIFIED_V2: bool = false;
const NEGACYCLIC_QUOTIENT_VERIFIED_V2: bool = false;
const PRIVATE_HYRAX_VERIFIED_V2: bool = false;
const Q_PCS_HANDOFF_COMPLETE_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const RELEASE_COMPLETE_V2: bool = false;
// Future algebra scratch is exactly 27 N-coefficient u64 owners plus three
// 8-KiB authenticated-read blocks.  The consumed root concurrently retains
// the scalar accumulator, compact manifests/authority/snapshot owners, and
// these scratch owners.  Provider internals, filesystem/kernel page cache,
// allocator metadata, and confidential/CAS backing storage are excluded.
const SOURCE_ALGEBRA_SCRATCH_U64_POLYNOMIALS_V2: usize = 27;
const SOURCE_ALGEBRA_READ_BLOCKS_V2: usize = 3;
const SOURCE_ALGEBRA_READ_BLOCK_BYTES_V2: usize = 8_192;
const SOURCE_ALGEBRA_LOCAL_SCRATCH_BYTES_V2: usize =
    SOURCE_ALGEBRA_SCRATCH_U64_POLYNOMIALS_V2 * PHASE23_RING_DEGREE_V1 * 8
        + SOURCE_ALGEBRA_READ_BLOCKS_V2 * SOURCE_ALGEBRA_READ_BLOCK_BYTES_V2;
const SOURCE_ALGEBRA_RETAINED_MATERIALIZED_BYTES_V2: usize = 50_383_680;
const SOURCE_ALGEBRA_RETAINED_COMPACT_MANIFEST_BYTES_V2: usize = 4_718_592;
const SOURCE_ALGEBRA_RETAINED_AUTHORITY_SNAPSHOT_BYTES_V2: usize = 65_536;
const SOURCE_ALGEBRA_WHOLE_NAMED_ROOT_BYTES_V2: usize =
    SOURCE_ALGEBRA_RETAINED_MATERIALIZED_BYTES_V2
        + SOURCE_ALGEBRA_RETAINED_COMPACT_MANIFEST_BYTES_V2
        + SOURCE_ALGEBRA_RETAINED_AUTHORITY_SNAPSHOT_BYTES_V2
        + SOURCE_ALGEBRA_LOCAL_SCRATCH_BYTES_V2;
const _: () = {
    assert!(SOURCE_ALGEBRA_RECORDS_V2 == PHASE23_RECORD_COUNT_V1);
    assert!(SOURCE_ALGEBRA_RECORDS_V2 == PHASE23_MANIFEST_CAPACITY_V1);
    assert!(SOURCE_ALGEBRA_LIMBS_V2 == RELEASE_MODULI_V1.len());
    assert!(SOURCE_ALGEBRA_RELATION_COORDINATES_V2 == 3_268);
    assert!(SOURCE_ALGEBRA_CHALLENGE_PAIRS_V2 == 190);
    assert!(SOURCE_ALGEBRA_PRODUCT_COEFFICIENTS_V2 == 262_144);
    assert!(SOURCE_ALGEBRA_P_COEFFICIENTS_V2 == 262_144);
    assert!(SOURCE_ALGEBRA_H_COEFFICIENTS_V2 == 131_072);
    assert!(SOURCE_ALGEBRA_LOCAL_SCRATCH_BYTES_V2 == 28_336_128);
    assert!(SOURCE_ALGEBRA_WHOLE_NAMED_ROOT_BYTES_V2 == 83_503_936);
    assert!(!SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V2);
    assert!(!SOURCE_ALGEBRA_VERIFIED_V2);
    assert!(!RADIX_PACKING_VERIFIED_V2);
    assert!(!RADIX_CARRY_VERIFIED_V2);
    assert!(!NEGACYCLIC_QUOTIENT_VERIFIED_V2);
    assert!(!PRIVATE_HYRAX_VERIFIED_V2);
    assert!(!Q_PCS_HANDOFF_COMPLETE_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_COMPLETE_V2);
};
/// Production cannot construct this move-only ordering proof.  A later,
/// purpose-specific transition must replace both impossible payloads.
pub(super) enum OrderedCiphertextBundleSealV2 {
    Production {
        ordered_43_ciphertexts: Infallible,
        move_only_key_authority: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Production cannot claim radix/quotient/Hyrax completion in this slice.
/// This authority is accepted only after authenticated source replay exists.
pub(super) enum RadixHyraxProofSealV2 {
    Production {
        packing: Infallible,
        radix_carry: Infallible,
        negacyclic_quotient: Infallible,
        hyrax_bgv_equality: Infallible,
        authenticated_replay: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
#[repr(u8)]
enum SourceEquationV2 {
    Constant = 0,
    Linear = 1,
}
impl SourceEquationV2 {
    const fn tag_v2(&self) -> u8 {
        match self {
            Self::Constant => 0,
            Self::Linear => 1,
        }
    }
}
#[repr(u8)]
enum SourceKeyPolynomialV2 {
    B = 1,
    A = 2,
}
impl SourceKeyPolynomialV2 {
    const fn tag_v2(&self) -> u8 {
        match self {
            Self::B => 1,
            Self::A => 2,
        }
    }
}
#[repr(u8)]
enum SourceErrorPolynomialV2 {
    E0 = 1,
    E1 = 2,
}
impl SourceErrorPolynomialV2 {
    const fn tag_v2(&self) -> u8 {
        match self {
            Self::E0 => 1,
            Self::E1 => 2,
        }
    }
}
#[repr(u8)]
enum SourceCiphertextPolynomialV2 {
    C0 = 1,
    C1 = 2,
}
impl SourceCiphertextPolynomialV2 {
    const fn tag_v2(&self) -> u8 {
        match self {
            Self::C0 => 1,
            Self::C1 => 2,
        }
    }
}
struct SourceEquationDescriptorV2 {
    equation: SourceEquationV2,
    key: SourceKeyPolynomialV2,
    error: SourceErrorPolynomialV2,
    ciphertext: SourceCiphertextPolynomialV2,
    delta: u8,
}
fn equation_descriptor_v2(equation: usize) -> Result<SourceEquationDescriptorV2, ZkAmsMkheErrorV1> {
    match equation {
        0 => Ok(SourceEquationDescriptorV2 {
            equation: SourceEquationV2::Constant,
            key: SourceKeyPolynomialV2::B,
            error: SourceErrorPolynomialV2::E0,
            ciphertext: SourceCiphertextPolynomialV2::C0,
            delta: 1,
        }),
        1 => Ok(SourceEquationDescriptorV2 {
            equation: SourceEquationV2::Linear,
            key: SourceKeyPolynomialV2::A,
            error: SourceErrorPolynomialV2::E1,
            ciphertext: SourceCiphertextPolynomialV2::C1,
            delta: 0,
        }),
        _ => Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    }
}
struct SourceRelationCoordinateV2 {
    ordinal: u16,
    family: u8,
    family_chunk: u16,
    family_chunk_count: u16,
    logical_value_count: u32,
    equation: SourceEquationDescriptorV2,
    limb: u8,
    modulus: u64,
}
fn relation_coordinate_v2(
    ordinal: u16,
    equation: usize,
    limb: usize,
) -> Result<SourceRelationCoordinateV2, ZkAmsMkheErrorV1> {
    let position = phase23_record_position_v1(ordinal)?;
    let descriptor = equation_descriptor_v2(equation)?;
    let modulus = *RELEASE_MODULI_V1
        .get(limb)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    Ok(SourceRelationCoordinateV2 {
        ordinal,
        family: position.family as u8,
        family_chunk: position.chunk_index,
        family_chunk_count: position.family_chunk_count,
        logical_value_count: position.logical_value_count,
        equation: descriptor,
        limb: u8::try_from(limb).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        modulus,
    })
}
fn absorb_coordinate_v2(hash: &mut Keccak256, coordinate: &SourceRelationCoordinateV2) {
    hash.update(&coordinate.ordinal.to_be_bytes());
    hash.update(&[coordinate.family]);
    hash.update(&coordinate.family_chunk.to_be_bytes());
    hash.update(&coordinate.family_chunk_count.to_be_bytes());
    hash.update(&coordinate.logical_value_count.to_be_bytes());
    hash.update(&[
        coordinate.equation.equation.tag_v2(),
        coordinate.equation.key.tag_v2(),
        coordinate.equation.error.tag_v2(),
        coordinate.equation.ciphertext.tag_v2(),
        coordinate.equation.delta,
        coordinate.limb,
    ]);
    hash.update(&coordinate.modulus.to_be_bytes());
}
fn mapping_digest_for_record_order_v2(
    record_order: &[u16; SOURCE_ALGEBRA_RECORDS_V2],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut seen = [false; SOURCE_ALGEBRA_RECORDS_V2];
    let mut hash = Keccak256::new();
    hash.update(SOURCE_ALGEBRA_MAPPING_DOMAIN_V2);
    hash.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    hash.update(&(SOURCE_ALGEBRA_RECORDS_V2 as u16).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_EQUATIONS_V2 as u16).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_LIMBS_V2 as u16).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_RELATION_COORDINATES_V2 as u32).to_be_bytes());
    hash.update(&(PHASE23_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for ordinal in record_order {
        let index = usize::from(*ordinal);
        if index >= seen.len() || seen[index] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen[index] = true;
        for equation in 0..SOURCE_ALGEBRA_EQUATIONS_V2 {
            for limb in 0..SOURCE_ALGEBRA_LIMBS_V2 {
                let coordinate = relation_coordinate_v2(*ordinal, equation, limb)?;
                absorb_coordinate_v2(&mut hash, &coordinate);
            }
        }
    }
    if seen.iter().any(|present| !present) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    nonzero_digest_v2(hash.finalize())
}
fn exact_mapping_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut record_order = [0_u16; SOURCE_ALGEBRA_RECORDS_V2];
    for (ordinal, destination) in record_order.iter_mut().enumerate() {
        *destination =
            u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    mapping_digest_for_record_order_v2(&record_order)
}
fn exact_formula_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_ALGEBRA_FORMULA_DOMAIN_V2);
    hash.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    hash.update(&(PHASE23_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_PRODUCT_COEFFICIENTS_V2 as u32).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_P_COEFFICIENTS_V2 as u32).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_H_COEFFICIENTS_V2 as u32).to_be_bytes());
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    for modulus in RELEASE_MODULI_V1 {
        hash.update(&modulus.to_be_bytes());
    }
    for formula in [
        ORDINARY_PRODUCT_FORMULA_V2,
        QUOTIENT_FORMULA_V2,
        RELATION_FORMULA_V2,
        TOP_ZERO_FORMULA_V2,
        LIMB_FORMULA_V2,
        CENTERING_FORMULA_V2,
        EQUATION_ZERO_FORMULA_V2,
        EQUATION_ONE_FORMULA_V2,
    ] {
        hash.update(&(formula.len() as u16).to_be_bytes());
        hash.update(formula);
    }
    nonzero_digest_v2(hash.finalize())
}
struct ManifestPreflightAxesV2 {
    ordered_bundle_root: [u8; 32],
    source_lineage_root: [u8; 32],
    output_lineage_root: [u8; 32],
    preflight_digest: [u8; 32],
}
fn require_common_snapshot_v2(
    receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    common: &mut Option<([u8; 32], [u8; 32])>,
    hash: &mut Keccak256,
) -> Result<(), ZkAmsMkheErrorV1> {
    let axes = streaming_source_snapshot_axes_v1(receipt);
    if axes.0 == [0; 32] || axes.1 == [0; 32] || receipt.receipt_digest() == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    match common {
        None => *common = Some(axes),
        Some(expected) if *expected == axes => {}
        Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
    }
    hash.update(&axes.0);
    hash.update(&axes.1);
    hash.update(&receipt.snapshot().pointer().pointer_digest());
    hash.update(&receipt.receipt_digest());
    Ok(())
}
fn require_common_output_v2(
    receipt: &ZkAmsMkheDirectObjectPublicationReceiptV1,
    publication_identity: &mut Option<[u8; 32]>,
    snapshot: &mut Option<([u8; 32], [u8; 32])>,
    hash: &mut Keccak256,
) -> Result<(), ZkAmsMkheErrorV1> {
    if receipt.publication_identity() == [0; 32] || receipt.receipt_digest() == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    match publication_identity {
        None => *publication_identity = Some(receipt.publication_identity()),
        Some(expected) if *expected == receipt.publication_identity() => {}
        Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
    }
    hash.update(&receipt.publication_identity());
    hash.update(&receipt.pointer().pointer_digest());
    hash.update(&receipt.receipt_digest());
    require_common_snapshot_v2(receipt.post_publish_read_receipt(), snapshot, hash)
}
fn exact_manifest_preflight_v2<K, P>(
    owner: &ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>,
) -> Result<ManifestPreflightAxesV2, ZkAmsMkheErrorV1> {
    owner.validate_v1()?;
    if owner.authority.failed
        || owner.manifests.len() != SOURCE_ALGEBRA_RECORDS_V2
        || owner.manifests.capacity() != SOURCE_ALGEBRA_RECORDS_V2
        || owner.authority.next_sample_index() != SOURCE_ALGEBRA_RECORDS_V2 as u64
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut ordered = Keccak256::new();
    ordered.update(SOURCE_ALGEBRA_ORDERED_BUNDLE_DOMAIN_V2);
    ordered.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    ordered.update(&owner.bundle_digest);
    ordered.update(&owner.source.receipt_v1().receipt_digest_v1());
    ordered.update(&(SOURCE_ALGEBRA_RECORDS_V2 as u16).to_be_bytes());
    let mut source_lineage = Keccak256::new();
    source_lineage.update(SOURCE_ALGEBRA_SOURCE_LINEAGE_DOMAIN_V2);
    source_lineage.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    let mut output_lineage = Keccak256::new();
    output_lineage.update(SOURCE_ALGEBRA_OUTPUT_LINEAGE_DOMAIN_V2);
    output_lineage.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    let mut common_source_snapshot = None;
    let mut common_output_snapshot = None;
    let mut common_output_publication = None;
    for (ordinal, manifest) in owner.manifests.iter().enumerate() {
        manifest.validate_for_authority_v1(&owner.authority)?;
        let binding = manifest.sealed_binding_v1()?;
        let ordinal_u16 =
            u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let position = phase23_record_position_v1(ordinal_u16)?;
        if binding.sample_index() != ordinal as u64
            || binding.level() != 0
            || binding.profile_digest() != owner.materialized.profile_digest
            || binding.roster_digest() != owner.materialized.roster_digest
            || binding.security_certificate_digest()
                != owner.authority.security_certificate_digest()
            || binding.key_material_digest() != owner.authority.key_material_digest()
            || binding.epoch() != owner.authority.epoch()
            || binding.key_transcript_digest() != owner.authority.transcript_digest()
            || binding.key_digest() != owner.authority.key_digest()
            || binding.key_authority_digest() != owner.authority.authority_digest()
            || manifest.topology.layout_digest != position.layout_v1()?.digest
            || manifest.topology.plaintext_chunk_index != u32::from(position.chunk_index)
            || manifest.topology.plaintext_used_slots != position.used_slots_v1()?
            || binding.constant_limb_pointers().len() != SOURCE_ALGEBRA_LIMBS_V2
            || binding.linear_limb_pointers().len() != SOURCE_ALGEBRA_LIMBS_V2
            || binding.constant_publication_receipts().len() != SOURCE_ALGEBRA_LIMBS_V2
            || binding.linear_publication_receipts().len() != SOURCE_ALGEBRA_LIMBS_V2
            || manifest.public_a_prepass_receipts.len() != SOURCE_ALGEBRA_LIMBS_V2
            || manifest.public_b_prepass_receipts.len() != SOURCE_ALGEBRA_LIMBS_V2
            || manifest.public_a_second_pass_receipts.len() != SOURCE_ALGEBRA_LIMBS_V2
            || manifest.public_b_second_pass_receipts.len() != SOURCE_ALGEBRA_LIMBS_V2
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        ordered.update(&ordinal_u16.to_be_bytes());
        ordered.update(&[position.family as u8]);
        ordered.update(&position.chunk_index.to_be_bytes());
        ordered.update(&position.family_chunk_count.to_be_bytes());
        ordered.update(&position.logical_value_count.to_be_bytes());
        ordered.update(&binding.manifest_digest());
        ordered.update(&binding.transcript_digest());
        ordered.update(&binding.ciphertext_digest());
        for receipts in [
            manifest.public_a_prepass_receipts.as_slice(),
            manifest.public_b_prepass_receipts.as_slice(),
            manifest.public_a_second_pass_receipts.as_slice(),
            manifest.public_b_second_pass_receipts.as_slice(),
        ] {
            for receipt in receipts {
                require_common_snapshot_v2(
                    receipt,
                    &mut common_source_snapshot,
                    &mut source_lineage,
                )?;
            }
        }
        for receipts in [
            binding.constant_publication_receipts(),
            binding.linear_publication_receipts(),
        ] {
            for receipt in receipts {
                require_common_output_v2(
                    receipt,
                    &mut common_output_publication,
                    &mut common_output_snapshot,
                    &mut output_lineage,
                )?;
            }
        }
    }
    if common_source_snapshot.is_none()
        || common_output_snapshot.is_none()
        || common_output_publication.is_none()
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let ordered_bundle_root = nonzero_digest_v2(ordered.finalize())?;
    let source_lineage_root = nonzero_digest_v2(source_lineage.finalize())?;
    let output_lineage_root = nonzero_digest_v2(output_lineage.finalize())?;
    let mut preflight = Keccak256::new();
    preflight.update(SOURCE_ALGEBRA_PREFLIGHT_DOMAIN_V2);
    preflight.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    preflight.update(&owner.materialized.profile_digest);
    preflight.update(&owner.materialized.roster_digest);
    preflight.update(&owner.materialized.transcript_digest);
    preflight.update(&owner.materialized.batch_id);
    preflight.update(&owner.materialized.ordered_batch_input_digest);
    preflight.update(&[owner.materialized.fold_count]);
    for value in [
        owner.materialized.shape.x,
        owner.materialized.shape.e,
        owner.materialized.shape.r_e,
        owner.materialized.shape.w,
        owner.materialized.shape.r_w,
    ] {
        preflight.update(&value.to_be_bytes());
    }
    preflight.update(&owner.materialized.digest);
    preflight.update(&owner.authority.key_digest());
    preflight.update(&owner.authority.authority_digest());
    preflight.update(&owner.authority.epoch().to_be_bytes());
    preflight.update(&owner.bundle_digest);
    preflight.update(&owner.source.receipt_v1().receipt_digest_v1());
    preflight.update(&ordered_bundle_root);
    preflight.update(&source_lineage_root);
    preflight.update(&output_lineage_root);
    let preflight_digest = nonzero_digest_v2(preflight.finalize())?;
    Ok(ManifestPreflightAxesV2 {
        ordered_bundle_root,
        source_lineage_root,
        output_lineage_root,
        preflight_digest,
    })
}
fn aggregate_schedule_digest_v2<K, P>(
    owner: &ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>,
    axes: &ManifestPreflightAxesV2,
    formula_digest: [u8; 32],
    mapping_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_ALGEBRA_AGGREGATE_SCHEDULE_DOMAIN_V2);
    hash.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    hash.update(&owner.bundle_digest);
    hash.update(&owner.source.receipt_v1().receipt_digest_v1());
    hash.update(&owner.materialized.digest);
    hash.update(&formula_digest);
    hash.update(&mapping_digest);
    hash.update(&axes.preflight_digest);
    for frame in [
        COMMITMENT_CHALLENGE_ORDER_V2,
        CHALLENGE_RULE_V2,
        AGGREGATE_FORMULA_V2,
        AGGREGATE_EQUIVALENT_FORMULA_V2,
        AGGREGATE_EMISSION_ORDER_V2,
    ] {
        hash.update(&(frame.len() as u16).to_be_bytes());
        hash.update(frame);
    }
    hash.update(&(SOURCE_ALGEBRA_LIMBS_V2 as u16).to_be_bytes());
    hash.update(&(SOURCE_ALGEBRA_AGGREGATE_REPETITIONS_V2 as u16).to_be_bytes());
    for (limb, modulus) in RELEASE_MODULI_V1.iter().enumerate() {
        for repetition in 0..SOURCE_ALGEBRA_AGGREGATE_REPETITIONS_V2 {
            hash.update(&(limb as u16).to_be_bytes());
            hash.update(&modulus.to_be_bytes());
            hash.update(&(repetition as u16).to_be_bytes());
            hash.update(b"gamma_lk");
            hash.update(b"beta_lk");
        }
    }
    nonzero_digest_v2(hash.finalize())
}
struct SourceAlgebraLiveV2<K, P> {
    owner: ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>,
    _ordered_ciphertexts: OrderedCiphertextBundleSealV2,
}
struct SourceAlgebraIngressV2<K, P> {
    live: Option<SourceAlgebraLiveV2<K, P>>,
}
struct SourceAlgebraPreflightV2<K, P> {
    live: Option<SourceAlgebraLiveV2<K, P>>,
    axes: ManifestPreflightAxesV2,
}
struct SourceAlgebraPrerequisiteRecordV2 {
    formula_digest: [u8; 32],
    mapping_digest: [u8; 32],
    ordered_bundle_root: [u8; 32],
    source_lineage_root: [u8; 32],
    output_lineage_root: [u8; 32],
    preflight_digest: [u8; 32],
    aggregate_schedule_digest: [u8; 32],
    source_relation_polynomials_constructed: bool,
    source_algebra_verified: bool,
    radix_packing_verified: bool,
    radix_carry_verified: bool,
    negacyclic_quotient_verified: bool,
    private_hyrax_verified: bool,
    q_pcs_handoff_complete: bool,
    operational_receipt_accepted: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}
/// Move-only pre-replay owner of the consumed Phase-23 bundle and its
/// false-gated prerequisite record. It makes no radix/Hyrax claim and has no
/// field accessors or decomposition seam.
pub(super) struct Phase23SourceAlgebraPrerequisiteV2<K, P> {
    live: Option<SourceAlgebraLiveV2<K, P>>,
    record: SourceAlgebraPrerequisiteRecordV2,
}
#[path = "incremental_source_phase23_source_algebra/global_lookup_source_replay_v1.rs"]
mod global_lookup_source_replay_v1;
pub(super) use global_lookup_source_replay_v1::{
    GlobalLookupProofSessionEntropySealV1, GlobalLookupSourceReplaySinkSealV1,
    Phase23GlobalLookupRadixSourceCursorV2, Phase23GlobalLookupSourceReplayEvidenceV1,
    Phase23GlobalLookupSourceReplayV1, bind_radix_hyrax_replay_after_materialization_v2,
};
impl<K, P> SourceAlgebraIngressV2<K, P> {
    fn begin_v2(
        owner: ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>,
        ordered_ciphertexts: OrderedCiphertextBundleSealV2,
    ) -> Self {
        Self {
            live: Some(SourceAlgebraLiveV2 {
                owner,
                _ordered_ciphertexts: ordered_ciphertexts,
            }),
        }
    }
    fn preflight_v2(mut self) -> Result<SourceAlgebraPreflightV2<K, P>, ZkAmsMkheErrorV1> {
        // The live owner is removed before the first validation.  Any error or
        // unwind drops it locally, so the caller can neither retry nor recover
        // an earlier manifest/CAS capability.
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let axes = exact_manifest_preflight_v2(&live.owner)?;
        Ok(SourceAlgebraPreflightV2 {
            live: Some(live),
            axes,
        })
    }
}
impl<K, P> SourceAlgebraPreflightV2<K, P> {
    fn freeze_v2(mut self) -> Result<Phase23SourceAlgebraPrerequisiteV2<K, P>, ZkAmsMkheErrorV1> {
        // As above, take precedes every revalidation and every future read
        // boundary represented by this prerequisite.
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.owner.validate_v1()?;
        if self.axes.preflight_digest == [0; 32]
            || self.axes.ordered_bundle_root == [0; 32]
            || self.axes.source_lineage_root == [0; 32]
            || self.axes.output_lineage_root == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let formula_digest = exact_formula_digest_v2()?;
        let mapping_digest = exact_mapping_digest_v2()?;
        let aggregate_schedule_digest =
            aggregate_schedule_digest_v2(&live.owner, &self.axes, formula_digest, mapping_digest)?;
        let mut record = SourceAlgebraPrerequisiteRecordV2 {
            formula_digest,
            mapping_digest,
            ordered_bundle_root: self.axes.ordered_bundle_root,
            source_lineage_root: self.axes.source_lineage_root,
            output_lineage_root: self.axes.output_lineage_root,
            preflight_digest: self.axes.preflight_digest,
            aggregate_schedule_digest,
            source_relation_polynomials_constructed: SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V2,
            source_algebra_verified: SOURCE_ALGEBRA_VERIFIED_V2,
            radix_packing_verified: RADIX_PACKING_VERIFIED_V2,
            radix_carry_verified: RADIX_CARRY_VERIFIED_V2,
            negacyclic_quotient_verified: NEGACYCLIC_QUOTIENT_VERIFIED_V2,
            private_hyrax_verified: PRIVATE_HYRAX_VERIFIED_V2,
            q_pcs_handoff_complete: Q_PCS_HANDOFF_COMPLETE_V2,
            operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V2,
            release_complete: RELEASE_COMPLETE_V2,
            record_digest: [0; 32],
        };
        record.record_digest = prerequisite_record_digest_v2(&record)?;
        validate_prerequisite_record_v2(&record)?;
        Ok(Phase23SourceAlgebraPrerequisiteV2 {
            live: Some(live),
            record,
        })
    }
}
impl<K, P> Phase23SourceAlgebraPrerequisiteV2<K, P> {
    /// Consume this exact prerequisite into authenticated compact signed-i8
    /// source planes. Production cannot supply the private sink authority yet.
    pub(super) fn into_global_lookup_source_replay_v1(
        self,
        sink: GlobalLookupSourceReplaySinkSealV1,
        proof_session_entropy: GlobalLookupProofSessionEntropySealV1,
    ) -> Result<Phase23GlobalLookupSourceReplayEvidenceV1<K, P>, ZkAmsMkheErrorV1> {
        global_lookup_source_replay_v1::replay_global_lookup_source_v1(
            self,
            sink,
            proof_session_entropy,
        )
    }
}
fn prerequisite_record_digest_v2(
    record: &SourceAlgebraPrerequisiteRecordV2,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_ALGEBRA_PREREQUISITE_DOMAIN_V2);
    hash.update(&[SOURCE_ALGEBRA_VERSION_V2]);
    hash.update(&record.formula_digest);
    hash.update(&record.mapping_digest);
    hash.update(&record.ordered_bundle_root);
    hash.update(&record.source_lineage_root);
    hash.update(&record.output_lineage_root);
    hash.update(&record.preflight_digest);
    hash.update(&record.aggregate_schedule_digest);
    hash.update(&[
        record.source_relation_polynomials_constructed as u8,
        record.source_algebra_verified as u8,
        record.radix_packing_verified as u8,
        record.radix_carry_verified as u8,
        record.negacyclic_quotient_verified as u8,
        record.private_hyrax_verified as u8,
        record.q_pcs_handoff_complete as u8,
        record.operational_receipt_accepted as u8,
        record.release_complete as u8,
    ]);
    nonzero_digest_v2(hash.finalize())
}
fn validate_prerequisite_record_v2(
    record: &SourceAlgebraPrerequisiteRecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    if [
        record.formula_digest,
        record.mapping_digest,
        record.ordered_bundle_root,
        record.source_lineage_root,
        record.output_lineage_root,
        record.preflight_digest,
        record.aggregate_schedule_digest,
        record.record_digest,
    ]
    .contains(&[0; 32])
        || record.source_relation_polynomials_constructed
        || record.source_algebra_verified
        || record.radix_packing_verified
        || record.radix_carry_verified
        || record.negacyclic_quotient_verified
        || record.private_hyrax_verified
        || record.q_pcs_handoff_complete
        || record.operational_receipt_accepted
        || record.release_complete
        || record.record_digest != prerequisite_record_digest_v2(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}
fn nonzero_digest_v2(digest: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}
pub(super) fn consume_phase23_source_algebra_prerequisite_v2<K, P>(
    owner: ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>,
    ordered_ciphertexts: OrderedCiphertextBundleSealV2,
) -> Result<Phase23SourceAlgebraPrerequisiteV2<K, P>, ZkAmsMkheErrorV1> {
    SourceAlgebraIngressV2::begin_v2(owner, ordered_ciphertexts)
        .preflight_v2()?
        .freeze_v2()
}
#[cfg(test)]
#[path = "incremental_source_phase23_source_algebra_tests.rs"]
mod tests;
