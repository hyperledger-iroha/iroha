//! Compiled-profile digests of the verifier-derived fixed tables that the SHA
//! and P-256 log-19 atom digests do not cover.
//!
//! MAIN and the CA subproof derive every fixed polynomial from public data at
//! verifier entry. Only the SHA and P-256 log-19 atom schedules entered the
//! compiled profile by digest, so a build that changed any other table, or the
//! DER closed forms that evaluate one, kept the same pin (gap 10 of
//! `specs/zk_x509_relation_coverage.md`). This module digests the rest, once
//! per process and before the pin comparison:
//!
//! - the DER, CA and P-256 log-5, log-8 and log-16 tables, which are constant;
//! - the RFC 5280 public affine schedule that MAIN evaluates, and the IO and
//!   Projection tables, each at five reference statements, one per disclosure
//!   count, because those tables also carry statement values.
//!
//! Two checks in the same pass tie the verifier's own evaluators to what is
//! digested: the DER closed forms, and the shifted affine sums over each
//! reference schedule, must equal direct interpolation at a fixed extension
//! point and at its next-row translate. A mismatch fails profile construction.

use super::main_oods::lagrange_weights_v1;
use super::*;
use crate::privacy_engines::zk_x509::accumulator_stark::{
    ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1, ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
    compile_ca_accumulator_fixed_columns_v1,
};
use crate::privacy_engines::zk_x509::der_air::ZkX509DerEkuV1;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    privacy::{
        PrivacyAttributeDigestV1, PrivacyCertificateKeyDigestV1, PrivacyChallengeV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyNullifierV1,
        PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyRootV1,
        PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
        PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1, PrivacyX509ExtendedKeyUsageV1,
        PrivacyX509KeyUsageRequirementV1, PrivacyX509KeyUsageV1,
        PrivacyZkX509CertificatePolicyRecordDigestV1, PrivacyZkX509CrlRecordDigestV1,
        PrivacyZkX509DisclosedAttributeV1, PrivacyZkX509TrustAnchorRecordDigestV1,
        ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1,
    },
};
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
#[path = "main_fixed_binding_tests.rs"]
mod tests;

/// Exact semantics of compiled-profile fields 29 to 35.
pub(crate) const ZK_X509_VERIFIER_FIXED_BINDING_DESCRIPTOR_V1: &[u8] = b"zk-x509-verifier-fixed-binding-v1:first-release-no-legacy:fields30-35=sha3-384-opaque48-of-der,rfc5280-public-affine,io,projection,p256-log5-8-16,ca:frame=privacy-outer-sha3-384-release-fixed-digest-context-role-zk-x509-verifier-fixed-binding-level0-counter0:cell=canonical-goldilocks-below-0xffffffff00000001-as-u64be:table=all-2^log2-rows-in-order-each-row-all-width-cells-in-column-order-hashed-in-blocks-of-min-4096-rows-phase-table-block-index-block-ordinal-fields-label-cells-then-phase-table-index0-fields-descriptor-label-log2-u8-width-u32be-concatenated-block-digests:rfc5280-member=phase-rfc5280-public-affine-schedule-index-disclosures-fields-descriptor-segment-count-u32be-segments-of-the-production-maximal-affine-schedule-of-rfc148-and-sha-public4x46-sorted-by-column-then-start-each-column-u16be-start-u32be-end-u32be-start-value-u64be-step-u64be:family=phase-family-index0-fields-descriptor-family-label-concatenated-member-digests:labels=der,rfc5280-public-affine,io-plus-disclosures-u8,projection-plus-disclosures-u8,p256-plus-layout-position-u16be-plus-instance-u16be,ca:references=five-source-fixed-statements-one-per-disclosure-count-0-4:p256=every-p256-registration-below-log19-in-layout-order:ca=log12-80-columns:ties=der-closed-forms-and-shifted-affine-sums-equal-direct-barycentric-interpolation-at-a-fixed-fp4-point-and-with-independent-weights-at-its-omega19-translate:once-per-process-before-pin-comparison";
/// Number of table digests that follow the descriptor in the compiled profile.
pub(crate) const ZK_X509_VERIFIER_FIXED_BINDING_DIGEST_COUNT_V1: usize = 6;
/// Ordered table digests: DER, RFC 5280 affine, IO, Projection, P-256, CA.
pub(crate) type ZkX509VerifierFixedBindingDigestsV1 =
    [[u8; 48]; ZK_X509_VERIFIER_FIXED_BINDING_DIGEST_COUNT_V1];

const BINDING_ROLE_V1: &[u8] = b"zk-x509-verifier-fixed-binding";
const BLOCK_ROWS_V1: usize = 4_096;
const DISCLOSURE_SHAPES_V1: usize = ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 + 1;
const AFFINE_SEGMENT_BYTES_V1: usize = 2 + 4 + 4 + 8 + 8;
const _: () = assert!(DISCLOSURE_SHAPES_V1 == 5);

/// Fixed extension point for the evaluator ties. It lies off every native
/// subgroup; `lagrange_weights_v1` rejects any point that does not.
const TIE_POINT_LIMBS_V1: [u64; 4] = [
    0x5f3a_9c2e_71d4_08b6,
    0x2c81_e6f9_4a07_d35b,
    0x7e16_4b93_c2a8_5f01,
    0x1d9b_7055_e3c4_a2f8,
];

static DIGESTS_V1: OnceLock<ZkX509VerifierFixedBindingDigestsV1> = OnceLock::new();
static COMPUTE_V1: Mutex<()> = Mutex::new(());

#[cfg(test)]
thread_local! {
    /// Test-only control: add one to the cell at `(row, column)` of the member
    /// table with this label while digesting it.
    static CELL_MUTATION_V1: core::cell::RefCell<Option<(Vec<u8>, usize, usize)>> =
        const { core::cell::RefCell::new(None) };
}
#[cfg(test)]
fn mutated_column_v1(label: &[u8], row: usize) -> Option<usize> {
    CELL_MUTATION_V1.with_borrow(|slot| {
        slot.as_ref()
            .filter(|(target, target_row, _)| target.as_slice() == label && *target_row == row)
            .map(|(_, _, column)| *column)
    })
}
/// One changed cell per bound table, as `(table, label, row, column)`, for the
/// pin controls. Each cell lies inside its table; the P-256 one is in the first
/// log-16 registration.
#[cfg(test)]
pub(crate) fn zk_x509_verifier_fixed_binding_cell_controls_v1()
-> Result<Vec<(usize, Vec<u8>, usize, usize)>, ZkX509StarkErrorV1> {
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    let (p256, _) = p256_members_v1(&layout)?
        .into_iter()
        .find(|(_, registration)| registration.segment.trace_log2 == 16)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    Ok(vec![
        (0, b"der".to_vec(), 1_000, FIX_PARSER),
        (1, shape_label_v1(b"rfc5280", 4)?, 300_000, 10),
        (2, shape_label_v1(b"io", 2)?, 70_000, 3),
        (3, shape_label_v1(b"projection", 1)?, 1_234, 5),
        (4, p256, 12_345, 7),
        (5, b"ca".to_vec(), 100, 10),
    ])
}
/// Recompute one table family with one cell changed, for the pin controls.
/// `table` is the family's index among fields 30 to 35.
#[cfg(test)]
pub(crate) fn zk_x509_verifier_fixed_binding_digest_with_changed_cell_v1(
    table: usize,
    label: &[u8],
    row: usize,
    column: usize,
) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    CELL_MUTATION_V1.with_borrow_mut(|slot| *slot = Some((label.to_vec(), row, column)));
    let layout = AggregateProofLayoutV1::for_full_profile_v1();
    let result = layout.and_then(|layout| match table {
        0 | 1 => {
            let weights = tie_weights_v1()?;
            if table == 0 {
                let der = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1);
                der_digest_v1(&weights, |row| {
                    der.fixed_row(row)
                        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)
                })
            } else {
                rfc_digest_v1(&weights)
            }
        }
        2 => io_digest_v1(&layout),
        3 => projection_digest_v1(&layout),
        4 => p256_digest_v1(&layout),
        5 => ca_digest_v1(),
        _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
    });
    CELL_MUTATION_V1.with_borrow_mut(|slot| *slot = None);
    result
}

/// The six table digests, computed once per process. Failures are not cached.
pub(crate) fn zk_x509_verifier_fixed_binding_digests_v1()
-> Result<&'static ZkX509VerifierFixedBindingDigestsV1, ZkX509StarkErrorV1> {
    if let Some(digests) = DIGESTS_V1.get() {
        return Ok(digests);
    }
    // Serialize the first computation so concurrent verifiers do not repeat it.
    let _guard = COMPUTE_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(digests) = DIGESTS_V1.get() {
        return Ok(digests);
    }
    let digests = compute_verifier_fixed_binding_digests_v1()?;
    Ok(DIGESTS_V1.get_or_init(|| digests))
}

fn compute_verifier_fixed_binding_digests_v1()
-> Result<ZkX509VerifierFixedBindingDigestsV1, ZkX509StarkErrorV1> {
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    // The tie weights are the largest allocation; release them before the
    // tables that need none.
    let (der, rfc) = {
        let weights = tie_weights_v1()?;
        let der = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1);
        (
            der_digest_v1(&weights, |row| {
                der.fixed_row(row)
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)
            })?,
            rfc_digest_v1(&weights)?,
        )
    };
    Ok([
        der,
        rfc,
        io_digest_v1(&layout)?,
        projection_digest_v1(&layout)?,
        p256_digest_v1(&layout)?,
        ca_digest_v1()?,
    ])
}

fn frame_v1(phase: &[u8], index: u64, fields: &[&[u8]]) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    Ok(privacy_outer_digest_frame_v1(
        ZK_X509_FIXED_DIGEST_CONTEXT_V1,
        BINDING_ROLE_V1,
        phase,
        0,
        index,
        0,
        fields,
    )
    .map_err(map_transparent_error_v1)?
    .to_bytes())
}

/// Digest the ordered member digests of one family.
fn family_v1(label: &[u8], members: &[[u8; 48]]) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let concatenated = members.concat();
    frame_v1(
        b"family",
        0,
        &[
            ZK_X509_VERIFIER_FIXED_BINDING_DESCRIPTOR_V1,
            label,
            &concatenated,
        ],
    )
}

/// Digest one native table: big-endian cells in row-major order, hashed in
/// blocks of 4,096 rows, then the ordered block digests under the header.
/// `observe` sees every canonical row before it is hashed.
fn table_digest_v1<R: AsRef<[F]>>(
    label: &[u8],
    trace_log2: u8,
    width: usize,
    mut row: impl FnMut(usize) -> Result<R, ZkX509StarkErrorV1>,
    mut observe: impl FnMut(usize, &[F]),
) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    if width == 0 || trace_log2 == 0 || trace_log2 > ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let width_u32 = u32::try_from(width).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let rows = 1_usize << trace_log2;
    let block_rows = rows.min(BLOCK_ROWS_V1);
    let block_bytes = block_rows
        .checked_mul(width)
        .and_then(|cells| cells.checked_mul(8))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut block = Vec::new();
    block
        .try_reserve_exact(block_bytes)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    let mut block_digests = Vec::new();
    block_digests
        .try_reserve_exact(rows / block_rows * 48)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for (block_index, start) in (0..rows).step_by(block_rows).enumerate() {
        block.clear();
        for index in start..start + block_rows {
            let values = row(index)?;
            let values = values.as_ref();
            #[cfg(test)]
            let changed;
            #[cfg(test)]
            let values = match mutated_column_v1(label, index) {
                Some(column) => {
                    let mut copy = values.to_vec();
                    if let Some(cell) = copy.get_mut(column) {
                        *cell = cell.add(F::ONE);
                    }
                    changed = copy;
                    changed.as_slice()
                }
                None => values,
            };
            if values.len() != width || values.iter().any(|value| !value.is_canonical()) {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            observe(index, values);
            for value in values {
                block.extend_from_slice(&value.0.to_be_bytes());
            }
        }
        let block_index =
            u64::try_from(block_index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        block_digests.extend_from_slice(&frame_v1(b"table-block", block_index, &[label, &block])?);
    }
    let mut header = [0_u8; 5];
    header[0] = trace_log2;
    header[1..].copy_from_slice(&width_u32.to_be_bytes());
    frame_v1(
        b"table",
        0,
        &[
            ZK_X509_VERIFIER_FIXED_BINDING_DESCRIPTOR_V1,
            label,
            &header,
            &block_digests,
        ],
    )
}

/// Lagrange weights of the log-19 domain at the tie point and its translate.
fn tie_weights_v1() -> Result<[Vec<E>; 2], ZkX509StarkErrorV1> {
    let point = E::canonical(TIE_POINT_LIMBS_V1).ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let root = goldilocks_primitive_root_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
        .map_err(map_transparent_error_v1)?;
    Ok([
        lagrange_weights_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, point)?,
        lagrange_weights_v1(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, point.mul_base(root))?,
    ])
}

/// Digest the DER table and require the closed forms MAIN evaluates to equal
/// its interpolant at the tie point (current row) and its translate (next row).
fn der_digest_v1<R: AsRef<[F]>>(
    weights: &[Vec<E>; 2],
    row: impl FnMut(usize) -> Result<R, ZkX509StarkErrorV1>,
) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let mut direct = [[E::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1]; 2];
    let digest = table_digest_v1(
        b"der",
        ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
        ZK_X509_DER_STARK_FIXED_WIDTH_V1,
        row,
        |index, values| {
            for (target, weights) in direct.iter_mut().zip(weights) {
                for (cell, value) in target.iter_mut().zip(values) {
                    if *value != F::ZERO {
                        *cell = cell.add(weights[index].mul_base(*value));
                    }
                }
            }
        },
    )?;
    let (prefix, _) = main_log19_weight_prefixes_v1(&weights[0])?;
    for (shift, expected) in direct.iter().enumerate() {
        if main_log19_der_fixed_opening_from_prefix_v1(&weights[0], &prefix, shift)? != *expected {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    family_v1(b"der", &[digest])
}

/// The five RFC 5280 reference statements, one per disclosure count. They
/// cover one to three EKUs, CRL numbers of one to nine DER content bytes and
/// every disclosure index.
fn rfc_reference_statement_v1(disclosures: usize) -> Option<ZkX509Rfc5280StatementV1> {
    use ZkX509DerEkuV1::{ClientAuthentication, DocumentSigning, WalletIdentity};
    let (start, length, key_usage, ekus, crl_number, indices): (
        u64,
        u64,
        u16,
        &[ZkX509DerEkuV1],
        u64,
        &[u8],
    ) = match disclosures {
        0 => (1_893_456_000, 300, 0x0001, &[ClientAuthentication], 1, &[]),
        1 => (1_924_992_123, 299, 0x0011, &[WalletIdentity], 0x80, &[2]),
        2 => (
            2_556_143_999,
            1,
            0x0003,
            &[ClientAuthentication, DocumentSigning],
            0xffff,
            &[0, 3],
        ),
        3 => (
            2_840_140_800,
            180,
            0x0005,
            &[DocumentSigning, WalletIdentity],
            0x0123_4567_89ab,
            &[1, 2, 3],
        ),
        4 => (
            4_102_444_799 - 300,
            300,
            0x0017,
            &[ClientAuthentication, DocumentSigning, WalletIdentity],
            u64::MAX,
            &[0, 1, 2, 3],
        ),
        _ => return None,
    };
    Some(ZkX509Rfc5280StatementV1 {
        presentation_not_before_unix_seconds: start,
        presentation_not_after_unix_seconds: start + length,
        leaf_key_usage: key_usage,
        leaf_extended_key_usages: ekus.to_vec(),
        crl_number,
        disclosed_attribute_indices: indices.to_vec(),
    })
}

/// Digest the public affine schedule MAIN compiles for each RFC reference
/// statement, and require its shifted affine sums to equal direct summation
/// of the same segments at the tie point and its translate.
fn rfc_digest_v1(weights: &[Vec<E>; 2]) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let (prefix, linear) = main_log19_weight_prefixes_v1(&weights[0])?;
    let mut members = Vec::with_capacity(DISCLOSURE_SHAPES_V1);
    for disclosures in 0..DISCLOSURE_SHAPES_V1 {
        let statement =
            rfc_reference_statement_v1(disclosures).ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let shape = ZkX509Rfc5280StarkShapeV1::from_statement(&statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let rfc_fixed = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(shape)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let sha_fixed = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        let schedule = MainLog19PublicFixedAffineScheduleV1::compile_v1(&rfc_fixed, &sha_fixed)?;
        affine_tie_v1(&schedule, weights, &prefix, &linear)?;
        #[cfg(test)]
        let schedule = {
            let mut schedule = schedule;
            let label = shape_label_v1(b"rfc5280", disclosures)?;
            let target = CELL_MUTATION_V1.with_borrow(|slot| {
                slot.as_ref()
                    .filter(|(target, _, _)| target.as_slice() == label.as_slice())
                    .map(|(_, row, column)| (*row, *column))
            });
            if let Some((row, column)) = target {
                tests::increment_affine_cell_v1(&mut schedule.segments, column, row);
            }
            schedule
        };
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(schedule.segments.len() * AFFINE_SEGMENT_BYTES_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for segment in &schedule.segments {
            encoded.extend_from_slice(&segment.column.to_be_bytes());
            encoded.extend_from_slice(&segment.start.to_be_bytes());
            encoded.extend_from_slice(&segment.end.to_be_bytes());
            encoded.extend_from_slice(&segment.start_value.0.to_be_bytes());
            encoded.extend_from_slice(&segment.step.0.to_be_bytes());
        }
        let count = u32::try_from(schedule.segments.len())
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let disclosures_u64 =
            u64::try_from(disclosures).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        members.push(frame_v1(
            b"rfc5280-public-affine-schedule",
            disclosures_u64,
            &[
                ZK_X509_VERIFIER_FIXED_BINDING_DESCRIPTOR_V1,
                &count.to_be_bytes(),
                &encoded,
            ],
        )?);
    }
    family_v1(b"rfc5280-public-affine", &members)
}

/// Require MAIN's shifted affine sums to equal direct interpolation of the
/// same segments, with independent weights for the next-row translate.
fn affine_tie_v1(
    schedule: &MainLog19PublicFixedAffineScheduleV1,
    weights: &[Vec<E>; 2],
    prefix: &[E],
    linear: &[E],
) -> Result<(), ZkX509StarkErrorV1> {
    schedule.validate_v1()?;
    let mut production = vec![[E::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1]; 2];
    let mut direct = vec![[E::ZERO; MAIN_LOG19_PUBLIC_FIXED_WIDTH_V1]; 2];
    for segment in &schedule.segments {
        let column = usize::from(segment.column);
        for (shift, row) in production.iter_mut().enumerate() {
            row[column] = row[column].add(main_log19_shifted_affine_segment_sum_v1(
                prefix, linear, shift, *segment,
            )?);
        }
        let mut value = segment.start_value;
        for index in segment.start..segment.end {
            let index = usize::try_from(index).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            for (row, weights) in direct.iter_mut().zip(weights) {
                row[column] = row[column].add(weights[index].mul_base(value));
            }
            value = value.add(segment.step);
        }
    }
    if production != direct {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}

/// Label of one per-shape member table.
fn shape_label_v1(family: &[u8], disclosures: usize) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    let mut label = family.to_vec();
    label.push(u8::try_from(disclosures).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?);
    Ok(label)
}

/// One reference byte string per tag, with distinct bytes in every position.
fn reference_bytes_v1(tag: u8) -> [u8; 32] {
    core::array::from_fn(|index| {
        tag.wrapping_mul(29)
            .wrapping_add(u8::try_from(index).unwrap_or(0).wrapping_mul(37))
            | 1
    })
}

/// The complete public statement for the IO and Projection references. Its
/// window, key usage, EKUs and disclosure indices match the RFC reference.
fn reference_statement_v1(
    disclosures: usize,
) -> Result<IrohaZkX509StarkP256StatementV1, ZkX509StarkErrorV1> {
    let rfc =
        rfc_reference_statement_v1(disclosures).ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
    let key_pair = KeyPair::try_from_seed(vec![0xa7; 32], Algorithm::Ed25519)
        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?;
    let flag = |bit: u16| -> PrivacyX509KeyUsageRequirementV1 {
        (rfc.leaf_key_usage & (1 << bit) != 0).into()
    };
    let usage = |eku: ZkX509DerEkuV1| match eku {
        ZkX509DerEkuV1::ClientAuthentication => PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
        ZkX509DerEkuV1::DocumentSigning => PrivacyX509ExtendedKeyUsageV1::DocumentSigning,
        ZkX509DerEkuV1::WalletIdentity => PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
    };
    Ok(IrohaZkX509StarkP256StatementV1 {
        context: PrivacyStatementContextV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(reference_bytes_v1(
                    0x01,
                ))),
            ),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(reference_bytes_v1(
                0x02,
            )),
            parameter_id: PrivacyParameterIdV1::new(reference_bytes_v1(0x03)),
            parameter_digest: PrivacyParameterDigestV1::new(reference_bytes_v1(0x04)),
            verifier_digest: PrivacyVerifierDigestV1::new(reference_bytes_v1(0x05)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(reference_bytes_v1(0x06)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(reference_bytes_v1(0x07)),
        },
        trust_anchor_id: PrivacyIssuerIdV1::new(reference_bytes_v1(0x08)),
        certificate_policy_id: PrivacyPolicyIdV1::new(reference_bytes_v1(0x09)),
        trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1::new(
            reference_bytes_v1(0x0a),
        ),
        trust_anchor_record_epoch: 3,
        certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1::new(
            reference_bytes_v1(0x0b),
        ),
        certificate_policy_record_epoch: 5,
        crl_record_digest: PrivacyZkX509CrlRecordDigestV1::new(reference_bytes_v1(0x0c)),
        crl_record_epoch: 7,
        subject_public_key_digest: PrivacyCertificateKeyDigestV1::new(reference_bytes_v1(0x0d)),
        ca_membership_root: PrivacyRootV1::new(reference_bytes_v1(0x0e)),
        ca_membership_root_epoch: 11,
        key_usage: PrivacyX509KeyUsageV1 {
            digital_signature: flag(0),
            content_commitment: flag(1),
            key_encipherment: flag(2),
            key_agreement: flag(4),
        },
        extended_key_usages: rfc
            .leaf_extended_key_usages
            .iter()
            .copied()
            .map(usage)
            .collect(),
        disclosed_attributes: rfc
            .disclosed_attribute_indices
            .iter()
            .map(|index| PrivacyZkX509DisclosedAttributeV1 {
                index: *index,
                attribute_digest: PrivacyAttributeDigestV1::new(reference_bytes_v1(
                    0x10_u8.wrapping_add(*index),
                )),
            })
            .collect(),
        presentation_not_before_unix_seconds: rfc.presentation_not_before_unix_seconds,
        presentation_not_after_unix_seconds: rfc.presentation_not_after_unix_seconds,
        wallet_account: AccountId::new(key_pair.public_key().clone()),
        wallet_challenge: PrivacyChallengeV1::new(reference_bytes_v1(0x14)),
        certificate_nullifier: PrivacyNullifierV1::new(reference_bytes_v1(0x15)),
    })
}

fn io_digest_v1(layout: &AggregateProofLayoutV1) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let registration = layout.registered_segment(SegmentAdapterIdV1::ByteMemory, 0)?;
    let mut members = Vec::with_capacity(DISCLOSURE_SHAPES_V1);
    for disclosures in 0..DISCLOSURE_SHAPES_V1 {
        let statement = reference_statement_v1(disclosures)?;
        let (_, schedule) = compile_main_io_fixed_schedule_v1(registration.segment, &statement)?;
        members.push(table_digest_v1(
            &shape_label_v1(b"io", disclosures)?,
            registration.segment.trace_log2,
            registration.segment.fixed_width,
            |row| schedule.fixed_row_v1(row),
            |_, _| {},
        )?);
    }
    family_v1(b"io", &members)
}

fn projection_digest_v1(layout: &AggregateProofLayoutV1) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let registration = layout.registered_segment(SegmentAdapterIdV1::Projection, 0)?;
    let mut members = Vec::with_capacity(DISCLOSURE_SHAPES_V1);
    for disclosures in 0..DISCLOSURE_SHAPES_V1 {
        let statement = reference_statement_v1(disclosures)?;
        let rows = compile_zk_x509_projection_stark_fixed_rows_v1(&statement)
            .map_err(|_| ZkX509StarkErrorV1::InvalidStatement)?;
        members.push(table_digest_v1(
            &shape_label_v1(b"projection", disclosures)?,
            registration.segment.trace_log2,
            registration.segment.fixed_width,
            |row| rows.get(row).ok_or(ZkX509StarkErrorV1::ProfileMismatch),
            |_, _| {},
        )?);
    }
    family_v1(b"projection", &members)
}

/// Label of one P-256 member table: its layout position and instance.
fn p256_label_v1(
    position: usize,
    registration: RegisteredSegmentLayoutV1,
) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
    let position = u16::try_from(position).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut label = b"p256".to_vec();
    label.extend_from_slice(&position.to_be_bytes());
    label.extend_from_slice(&registration.segment.instance.to_be_bytes());
    Ok(label)
}

/// Every P-256 registration below log 19, in layout order, with its label.
/// The log-19 tables are bound by their atom schedule in field 28.
fn p256_members_v1(
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<(Vec<u8>, RegisteredSegmentLayoutV1)>, ZkX509StarkErrorV1> {
    let mut members = Vec::new();
    for (position, registration) in layout.registered_segments.iter().copied().enumerate() {
        if matches!(
            MainFp4AirEvaluatorV1::for_registration_v1(registration)?,
            Some(MainFp4AirEvaluatorV1::P256(_))
        ) && registration.segment.trace_log2 != ZK_X509_MAX_NATIVE_TRACE_LOG2_V1
        {
            members.push((p256_label_v1(position, registration)?, registration));
        }
    }
    Ok(members)
}

/// Digest every P-256 table below log 19, in layout order.
fn p256_digest_v1(layout: &AggregateProofLayoutV1) -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let source = P256MainVerifierFixedSourceV1::new_v1().map_err(ZkX509StarkErrorV1::from)?;
    let mut members = Vec::new();
    for (label, registration) in p256_members_v1(layout)? {
        let identity = p256_main_registration_from_main_layout_v1(registration)?;
        members.push(table_digest_v1(
            &label,
            registration.segment.trace_log2,
            registration.segment.fixed_width,
            |row| {
                source
                    .fixed_row_v1(identity, row)
                    .map_err(ZkX509StarkErrorV1::from)
            },
            |_, _| {},
        )?);
    }
    if members.is_empty() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    family_v1(b"p256-log5-8-16", &members)
}

fn ca_digest_v1() -> Result<[u8; 48], ZkX509StarkErrorV1> {
    let columns = compile_ca_accumulator_fixed_columns_v1()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    if columns.len() != ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1 {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let digest = table_digest_v1(
        b"ca",
        ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
        |row| {
            columns
                .iter()
                .map(|column| column.get(row).copied())
                .collect::<Option<Vec<_>>>()
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
        },
        |_, _| {},
    )?;
    family_v1(b"ca", &[digest])
}
