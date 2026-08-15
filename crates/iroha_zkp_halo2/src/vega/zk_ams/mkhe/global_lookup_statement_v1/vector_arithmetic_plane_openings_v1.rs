//! Fail-closed retained-opening prerequisite for the three quadratic residuals.
//!
//! This child freezes the exact plane inventory, axes, authenticated-snapshot
//! layout, source-context binding, and one-shot replay purposes needed before
//! the `s = 3, 5, 8` vector-arithmetic proofs can be implemented.  A narrowly
//! purpose-bound confidential-spool constructor now admits the exact snapshot, but no upstream
//! owner supplies all values, blindings, and matching commitments. Production construction is
//! therefore deliberately uninhabited. No proof, transcript frame, wire byte, receipt, authority,
//! RSS claim, or release gate is added here.

#![allow(
    dead_code,
    reason = "the authenticated production snapshot seal is uninhabited"
)]
#![cfg_attr(
    not(test),
    allow(
        unreachable_code,
        unused_variables,
        reason = "the authenticated production snapshot seal is intentionally uninhabited"
    )
)]

use core::convert::Infallible;

use iroha_confidential_spool::{
    CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_FILE_BYTES_V1,
    CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_PLAINTEXT_BYTES_V1,
    CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_SLOTS_V1, CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1,
    CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1, CONFIDENTIAL_SPOOL_MAX_SLOTS_V1,
    ConfidentialSpoolLayoutV1,
};

use crate::vega::{bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, sponge::Keccak256};

use super::{challenge_v1::challenge_manifest_digest_v1, global_lookup_topology_digest_v1};

const PLANE_OPENING_VERSION_V1: u8 = 1;
const COORDINATES_PER_PLANE_V1: usize = 1 << 14;
const COMPARATOR_GROUPS_V1: usize = 344;
const SIGNED_SOURCE_UNITS_V1: usize = 1_032;
const BETA_COLUMNS_V1: usize = 18;
const QUADRATIC_STATEMENTS_V1: [u8; 3] = [3, 5, 8];

const BD_PLANES_V1: usize = COMPARATOR_GROUPS_V1;
const BS_PLANES_V1: usize = COMPARATOR_GROUPS_V1;
const BETA_PLANES_V1: usize = COMPARATOR_GROUPS_V1 * BETA_COLUMNS_V1;
const M_PLANES_V1: usize = COMPARATOR_GROUPS_V1;
const X_PLANES_V1: usize = SIGNED_SOURCE_UNITS_V1;
const N_PLANES_V1: usize = SIGNED_SOURCE_UNITS_V1;
const Q_PLANES_V1: usize = QUADRATIC_STATEMENTS_V1.len();
const COMMITMENT_MASKS_V1: usize = PLANE_COUNT_V1;

const BD_START_V1: usize = 0;
const BS_START_V1: usize = BD_START_V1 + BD_PLANES_V1;
const BETA_START_V1: usize = BS_START_V1 + BS_PLANES_V1;
const M_START_V1: usize = BETA_START_V1 + BETA_PLANES_V1;
const X_START_V1: usize = M_START_V1 + M_PLANES_V1;
const N_START_V1: usize = X_START_V1 + X_PLANES_V1;
const Q3_ORDINAL_V1: usize = N_START_V1 + N_PLANES_V1;
const Q5_ORDINAL_V1: usize = Q3_ORDINAL_V1 + 1;
const Q8_ORDINAL_V1: usize = Q5_ORDINAL_V1 + 1;
const PLANE_COUNT_V1: usize = Q8_ORDINAL_V1 + 1;

const SCALAR_BYTES_V1: u64 = 32;
const POINT_BYTES_V1: u64 = 33;
const SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1: u64 = 16_384;
const SNAPSHOT_SLOT_TAG_BYTES_V1: u64 = 16;
const VALUE_SLOTS_PER_PLANE_V1: u64 = 32;
const TAIL_SLOTS_PER_PLANE_V1: u64 = 1;
const SNAPSHOT_SLOTS_PER_PLANE_V1: u64 = VALUE_SLOTS_PER_PLANE_V1 + TAIL_SLOTS_PER_PLANE_V1;
const SNAPSHOT_SLOT_COUNT_V1: u64 = PLANE_COUNT_V1 as u64 * SNAPSHOT_SLOTS_PER_PLANE_V1;
const VALUE_BYTES_PER_PLANE_V1: u64 = COORDINATES_PER_PLANE_V1 as u64 * SCALAR_BYTES_V1;
const TAIL_SEMANTIC_BYTES_PER_PLANE_V1: u64 = SCALAR_BYTES_V1 + POINT_BYTES_V1;
const SNAPSHOT_SEMANTIC_BYTES_V1: u64 =
    PLANE_COUNT_V1 as u64 * (VALUE_BYTES_PER_PLANE_V1 + TAIL_SEMANTIC_BYTES_PER_PLANE_V1);
const SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1: u64 =
    SNAPSHOT_SLOT_COUNT_V1 * SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1;
const SNAPSHOT_FILE_BYTES_V1: u64 =
    SNAPSHOT_SLOT_COUNT_V1 * (SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 + SNAPSHOT_SLOT_TAG_BYTES_V1);
const SNAPSHOT_GENERAL_FILE_CAP_EXCESS_BYTES_V1: u64 =
    SNAPSHOT_FILE_BYTES_V1 - CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1;
const SNAPSHOT_AUTHENTICATION_TAG_BYTES_V1: u64 =
    SNAPSHOT_SLOT_COUNT_V1 * SNAPSHOT_SLOT_TAG_BYTES_V1;
const SNAPSHOT_ZERO_PADDING_BYTES_V1: u64 =
    SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1 - SNAPSHOT_SEMANTIC_BYTES_V1;
const RETAINED_VALUE_BYTES_V1: u64 = PLANE_COUNT_V1 as u64 * VALUE_BYTES_PER_PLANE_V1;
const RETAINED_BLINDING_BYTES_V1: u64 = PLANE_COUNT_V1 as u64 * SCALAR_BYTES_V1;
const RETAINED_COMMITMENT_WIRE_BYTES_V1: u64 = PLANE_COUNT_V1 as u64 * POINT_BYTES_V1;

const PLANE_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-plane.mapping\0";
const PLANE_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-plane.context\0";
const PLANE_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.vector-arithmetic-plane.record\0";
const GROUP_AXIS_LANGUAGE_V1: &[u8] =
    b"group=record*8+group-in-record;record=0..42;group-in-record=0..7";
const UNIT_AXIS_LANGUAGE_V1: &[u8] =
    b"unit=((record*3+signed-role)*8+plane);record=0..42;signed-role=(r,e0,e1);plane=0..7";
const COLUMN_AXIS_LANGUAGE_V1: &[u8] = b"beta-column=0..17;beta-order=group-major-then-column";
const COORDINATE_AXIS_LANGUAGE_V1: &[u8] =
    b"coordinate-v=0..16383;Boolean-coordinate-bits-little-endian;canonical-T256-scalar-big-endian-32";
const PLANE_ORDER_LANGUAGE_V1: &[u8] =
    b"plane-order=bD[group],bS[group],beta[group][column],m[group],x[unit],n[unit],q3,q5,q8;commitment-order=blinding-order=plane-order";
const SNAPSHOT_LAYOUT_LANGUAGE_V1: &[u8] =
    b"one-authenticated-confidential-snapshot;plane-major;per-plane-slots=value-chunk[0..31],tail;value-chunk=512-canonical-scalars;tail=blinding32||nonidentity-commitment33||zero-padding16319;slot=plane*33+local;no-independent-snapshot-authorities";
const COMMITMENT_LANGUAGE_V1: &[u8] =
    b"commitment-mask[plane]=blinding[plane];mask-order=plane-order;C_plane=sum_v(value[plane,v]*G[v])+blinding[plane]*H;one-nonzero-canonical-blinding-and-one-canonical-nonidentity-33B-point-per-plane;basis=ZkAmsT256BulletproofSuiteV1:G[0..16384)+H";
const SOURCE_CONTEXT_LANGUAGE_V1: &[u8] =
    b"context-order=topology,challenge-manifest,basis,mapping,source-replay-record,source-opening-record,canonical-reopen-record,radix-range-record,coefficient-residual-manifest,committed-MLE-profile";
const PRODUCTION_BLOCKER_LANGUAGE_V1: &[u8] =
    b"current-upstream-does-not-own-one-authenticated-snapshot-containing-all-9291-exact-values,blindings,and-matching-commitments;exact-purpose-specific-spool-geometry-is-available;production-seal-remains-Infallible";

const CURRENT_UPSTREAM_COMPLETE_V1: bool = false;
const CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1: bool = true;
const PLANE_OPENING_MATERIALIZED_V1: bool = false;
const TRANSCRIPT_FRAMES_ADDED_V1: usize = 0;
const WIRE_BYTES_ADDED_V1: usize = 0;
const KAT_ORDINALS_CHANGED_V1: bool = false;
const VECTOR_ARITHMETIC_PROOFS_WIRED_V1: bool = false;
const VECTOR_ARITHMETIC_PROOFS_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const AUTHORITY_MINTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(COORDINATES_PER_PLANE_V1 == 16_384);
    assert!(BETA_PLANES_V1 == 6_192);
    assert!(PLANE_COUNT_V1 == 9_291);
    assert!(COMMITMENT_MASKS_V1 == 9_291);
    assert!(
        PLANE_COUNT_V1
            == BD_PLANES_V1
                + BS_PLANES_V1
                + BETA_PLANES_V1
                + M_PLANES_V1
                + X_PLANES_V1
                + N_PLANES_V1
                + Q_PLANES_V1
    );
    assert!(VALUE_BYTES_PER_PLANE_V1 == 524_288);
    assert!(SNAPSHOT_SLOT_COUNT_V1 == 306_603);
    assert!(RETAINED_VALUE_BYTES_V1 == 4_871_159_808);
    assert!(RETAINED_BLINDING_BYTES_V1 == 297_312);
    assert!(RETAINED_COMMITMENT_WIRE_BYTES_V1 == 306_603);
    assert!(SNAPSHOT_SEMANTIC_BYTES_V1 == 4_871_763_723);
    assert!(SNAPSHOT_ZERO_PADDING_BYTES_V1 == 151_619_829);
    assert!(SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1 == 5_023_383_552);
    assert!(SNAPSHOT_AUTHENTICATION_TAG_BYTES_V1 == 4_905_648);
    assert!(SNAPSHOT_FILE_BYTES_V1 == 5_028_289_200);
    assert!(SNAPSHOT_GENERAL_FILE_CAP_EXCESS_BYTES_V1 == 1_198_764_720);
    assert!(SNAPSHOT_SLOT_COUNT_V1 <= CONFIDENTIAL_SPOOL_MAX_SLOTS_V1);
    assert!(SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 <= CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1);
    assert!(SNAPSHOT_FILE_BYTES_V1 > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!(SNAPSHOT_SLOT_COUNT_V1 == CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_SLOTS_V1);
    assert!(
        SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1
            == CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_PLAINTEXT_BYTES_V1
    );
    assert!(SNAPSHOT_FILE_BYTES_V1 == CONFIDENTIAL_SPOOL_GLOBAL_LOOKUP_PLANE_FILE_BYTES_V1);
    assert!(!CURRENT_UPSTREAM_COMPLETE_V1);
    assert!(CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1);
    assert!(!PLANE_OPENING_MATERIALIZED_V1);
    assert!(TRANSCRIPT_FRAMES_ADDED_V1 == 0 && WIRE_BYTES_ADDED_V1 == 0);
    assert!(!KAT_ORDINALS_CHANGED_V1);
    assert!(!VECTOR_ARITHMETIC_PROOFS_WIRED_V1);
    assert!(!VECTOR_ARITHMETIC_PROOFS_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!AUTHORITY_MINTED_V1);
    assert!(!RSS_QUALIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaneOpeningErrorV1 {
    Shape,
    Order,
    Context,
    Replay,
    Resource,
    Source,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalLookupPlaneRoleV1 {
    BooleanD = 1,
    BooleanS = 2,
    ComparatorBorrow = 3,
    MixedTop = 4,
    SmallSigned = 5,
    SmallNegativeMagnitude = 6,
    ResidualQ3 = 7,
    ResidualQ5 = 8,
    ResidualQ8 = 9,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlobalLookupPlaneCoordinateV1 {
    ordinal: u16,
    role: GlobalLookupPlaneRoleV1,
    group: Option<u16>,
    unit: Option<u16>,
    column: Option<u8>,
    statement: Option<u8>,
}

fn plane_coordinate_v1(
    ordinal: usize,
) -> Result<GlobalLookupPlaneCoordinateV1, PlaneOpeningErrorV1> {
    let (role, group, unit, column, statement) = match ordinal {
        BD_START_V1..BS_START_V1 => (
            GlobalLookupPlaneRoleV1::BooleanD,
            Some(ordinal - BD_START_V1),
            None,
            None,
            None,
        ),
        BS_START_V1..BETA_START_V1 => (
            GlobalLookupPlaneRoleV1::BooleanS,
            Some(ordinal - BS_START_V1),
            None,
            None,
            None,
        ),
        BETA_START_V1..M_START_V1 => {
            let local = ordinal - BETA_START_V1;
            (
                GlobalLookupPlaneRoleV1::ComparatorBorrow,
                Some(local / BETA_COLUMNS_V1),
                None,
                Some(local % BETA_COLUMNS_V1),
                None,
            )
        }
        M_START_V1..X_START_V1 => (
            GlobalLookupPlaneRoleV1::MixedTop,
            Some(ordinal - M_START_V1),
            None,
            None,
            None,
        ),
        X_START_V1..N_START_V1 => (
            GlobalLookupPlaneRoleV1::SmallSigned,
            None,
            Some(ordinal - X_START_V1),
            None,
            None,
        ),
        N_START_V1..Q3_ORDINAL_V1 => (
            GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
            None,
            Some(ordinal - N_START_V1),
            None,
            None,
        ),
        Q3_ORDINAL_V1 => (
            GlobalLookupPlaneRoleV1::ResidualQ3,
            None,
            None,
            None,
            Some(3),
        ),
        Q5_ORDINAL_V1 => (
            GlobalLookupPlaneRoleV1::ResidualQ5,
            None,
            None,
            None,
            Some(5),
        ),
        Q8_ORDINAL_V1 => (
            GlobalLookupPlaneRoleV1::ResidualQ8,
            None,
            None,
            None,
            Some(8),
        ),
        _ => return Err(PlaneOpeningErrorV1::Shape),
    };
    Ok(GlobalLookupPlaneCoordinateV1 {
        ordinal: u16::try_from(ordinal).map_err(|_| PlaneOpeningErrorV1::Resource)?,
        role,
        group: group
            .map(u16::try_from)
            .transpose()
            .map_err(|_| PlaneOpeningErrorV1::Resource)?,
        unit: unit
            .map(u16::try_from)
            .transpose()
            .map_err(|_| PlaneOpeningErrorV1::Resource)?,
        column: column
            .map(u8::try_from)
            .transpose()
            .map_err(|_| PlaneOpeningErrorV1::Resource)?,
        statement,
    })
}

fn role_plane_count_v1(role: GlobalLookupPlaneRoleV1) -> usize {
    match role {
        GlobalLookupPlaneRoleV1::BooleanD => BD_PLANES_V1,
        GlobalLookupPlaneRoleV1::BooleanS => BS_PLANES_V1,
        GlobalLookupPlaneRoleV1::ComparatorBorrow => BETA_PLANES_V1,
        GlobalLookupPlaneRoleV1::MixedTop => M_PLANES_V1,
        GlobalLookupPlaneRoleV1::SmallSigned => X_PLANES_V1,
        GlobalLookupPlaneRoleV1::SmallNegativeMagnitude => N_PLANES_V1,
        GlobalLookupPlaneRoleV1::ResidualQ3
        | GlobalLookupPlaneRoleV1::ResidualQ5
        | GlobalLookupPlaneRoleV1::ResidualQ8 => 1,
    }
}

fn absorb_len_prefixed_v1(hash: &mut Keccak256, bytes: &[u8]) -> Result<(), PlaneOpeningErrorV1> {
    let len = u16::try_from(bytes.len()).map_err(|_| PlaneOpeningErrorV1::Resource)?;
    hash.update(&len.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

fn plane_mapping_digest_v1() -> Result<[u8; 32], PlaneOpeningErrorV1> {
    let topology_digest = require_nonzero_v1(global_lookup_topology_digest_v1())?;
    let mut hash = Keccak256::new();
    hash.update(PLANE_MAPPING_DOMAIN_V1);
    hash.update(&[PLANE_OPENING_VERSION_V1]);
    hash.update(&topology_digest);
    for value in [
        COORDINATES_PER_PLANE_V1,
        COMPARATOR_GROUPS_V1,
        SIGNED_SOURCE_UNITS_V1,
        BETA_COLUMNS_V1,
        PLANE_COUNT_V1,
    ] {
        hash.update(&(value as u64).to_be_bytes());
    }
    for language in [
        GROUP_AXIS_LANGUAGE_V1,
        UNIT_AXIS_LANGUAGE_V1,
        COLUMN_AXIS_LANGUAGE_V1,
        COORDINATE_AXIS_LANGUAGE_V1,
        PLANE_ORDER_LANGUAGE_V1,
        SNAPSHOT_LAYOUT_LANGUAGE_V1,
        COMMITMENT_LANGUAGE_V1,
    ] {
        absorb_len_prefixed_v1(&mut hash, language)?;
    }
    for ordinal in 0..PLANE_COUNT_V1 {
        let coordinate = plane_coordinate_v1(ordinal)?;
        hash.update(&coordinate.ordinal.to_be_bytes());
        hash.update(&[coordinate.role as u8]);
        hash.update(&coordinate.group.unwrap_or(u16::MAX).to_be_bytes());
        hash.update(&coordinate.unit.unwrap_or(u16::MAX).to_be_bytes());
        hash.update(&[coordinate.column.unwrap_or(u8::MAX)]);
        hash.update(&[coordinate.statement.unwrap_or(0)]);
    }
    require_nonzero_v1(hash.finalize())
}

#[derive(Clone, Copy)]
struct PlaneOpeningSourceContextV1 {
    source_replay_record_digest: [u8; 32],
    source_opening_record_digest: [u8; 32],
    canonical_reopen_record_digest: [u8; 32],
    radix_range_record_digest: [u8; 32],
    coefficient_residual_manifest_digest: [u8; 32],
    committed_mle_profile_digest: [u8; 32],
}

fn plane_context_digest_v1(
    axes: PlaneOpeningSourceContextV1,
) -> Result<[u8; 32], PlaneOpeningErrorV1> {
    let topology_digest = require_nonzero_v1(global_lookup_topology_digest_v1())?;
    let challenge_digest = require_nonzero_v1(challenge_manifest_digest_v1())?;
    let mapping_digest = plane_mapping_digest_v1()?;
    let mut hash = Keccak256::new();
    hash.update(PLANE_CONTEXT_DOMAIN_V1);
    hash.update(&[PLANE_OPENING_VERSION_V1]);
    for digest in [
        topology_digest,
        challenge_digest,
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
        mapping_digest,
        axes.source_replay_record_digest,
        axes.source_opening_record_digest,
        axes.canonical_reopen_record_digest,
        axes.radix_range_record_digest,
        axes.coefficient_residual_manifest_digest,
        axes.committed_mle_profile_digest,
    ] {
        hash.update(&require_nonzero_v1(digest)?);
    }
    absorb_len_prefixed_v1(&mut hash, SOURCE_CONTEXT_LANGUAGE_V1)?;
    absorb_len_prefixed_v1(&mut hash, PRODUCTION_BLOCKER_LANGUAGE_V1)?;
    require_nonzero_v1(hash.finalize())
}

fn approved_snapshot_layout_v1(
    context_digest: [u8; 32],
) -> Result<ConfidentialSpoolLayoutV1, PlaneOpeningErrorV1> {
    let layout = ConfidentialSpoolLayoutV1::global_lookup_plane_openings_v1(context_digest)
        .map_err(|_| PlaneOpeningErrorV1::Resource)?;
    if layout.slot_count_v1() != SNAPSHOT_SLOT_COUNT_V1
        || layout.plaintext_len_v1() != SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1
        || layout.file_len_v1() != SNAPSHOT_FILE_BYTES_V1
    {
        return Err(PlaneOpeningErrorV1::Resource);
    }
    Ok(layout)
}

struct PlaneOpeningRecordV1 {
    topology_digest: [u8; 32],
    challenge_manifest_digest: [u8; 32],
    basis_digest: [u8; 32],
    mapping_digest: [u8; 32],
    context_digest: [u8; 32],
    source_context_axes: PlaneOpeningSourceContextV1,
    authenticated_snapshot_digest: [u8; 32],
    commitment_inventory_digest: [u8; 32],
    record_digest: [u8; 32],
}

fn plane_record_digest_v1(record: &PlaneOpeningRecordV1) -> Result<[u8; 32], PlaneOpeningErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(PLANE_RECORD_DOMAIN_V1);
    hash.update(&[PLANE_OPENING_VERSION_V1]);
    for digest in [
        record.topology_digest,
        record.challenge_manifest_digest,
        record.basis_digest,
        record.mapping_digest,
        record.context_digest,
        record.source_context_axes.source_replay_record_digest,
        record.source_context_axes.source_opening_record_digest,
        record.source_context_axes.canonical_reopen_record_digest,
        record.source_context_axes.radix_range_record_digest,
        record
            .source_context_axes
            .coefficient_residual_manifest_digest,
        record.source_context_axes.committed_mle_profile_digest,
        record.authenticated_snapshot_digest,
        record.commitment_inventory_digest,
    ] {
        hash.update(&require_nonzero_v1(digest)?);
    }
    for value in [
        COORDINATES_PER_PLANE_V1 as u64,
        PLANE_COUNT_V1 as u64,
        SNAPSHOT_SLOTS_PER_PLANE_V1,
        SNAPSHOT_SLOT_COUNT_V1,
        SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
        SNAPSHOT_SEMANTIC_BYTES_V1,
        SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1,
        SNAPSHOT_FILE_BYTES_V1,
        SNAPSHOT_GENERAL_FILE_CAP_EXCESS_BYTES_V1,
        RETAINED_VALUE_BYTES_V1,
        RETAINED_BLINDING_BYTES_V1,
        RETAINED_COMMITMENT_WIRE_BYTES_V1,
        TRANSCRIPT_FRAMES_ADDED_V1 as u64,
        WIRE_BYTES_ADDED_V1 as u64,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&[
        CURRENT_UPSTREAM_COMPLETE_V1 as u8,
        CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1 as u8,
        PLANE_OPENING_MATERIALIZED_V1 as u8,
        KAT_ORDINALS_CHANGED_V1 as u8,
        VECTOR_ARITHMETIC_PROOFS_WIRED_V1 as u8,
        VECTOR_ARITHMETIC_PROOFS_VERIFIED_V1 as u8,
        ZERO_KNOWLEDGE_ACCEPTED_V1 as u8,
        OPERATIONAL_RECEIPT_ACCEPTED_V1 as u8,
        AUTHORITY_MINTED_V1 as u8,
        RSS_QUALIFIED_V1 as u8,
        RELEASE_READY_V1 as u8,
    ]);
    require_nonzero_v1(hash.finalize())
}

fn validate_plane_record_v1(record: &PlaneOpeningRecordV1) -> Result<(), PlaneOpeningErrorV1> {
    if record.topology_digest != global_lookup_topology_digest_v1()
        || record.challenge_manifest_digest != challenge_manifest_digest_v1()
        || record.basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
        || record.mapping_digest != plane_mapping_digest_v1()?
        || record.context_digest != plane_context_digest_v1(record.source_context_axes)?
        || record.authenticated_snapshot_digest == [0; 32]
        || record.commitment_inventory_digest == [0; 32]
        || record.record_digest != plane_record_digest_v1(record)?
    {
        return Err(PlaneOpeningErrorV1::Context);
    }
    Ok(())
}

/// Production cannot supply this capability until one authenticated snapshot
/// owns all exact values and blindings and authenticates the matching points.
enum GlobalLookupPlaneOpeningMaterializerSealV1 {
    Production {
        authenticated_confidential_snapshot: Infallible,
        exact_plane_values: Infallible,
        exact_commitment_blindings: Infallible,
        exact_commitment_inventory: Infallible,
        authenticated_source_context: Infallible,
    },
    #[cfg(test)]
    TestOnly(TestAuthenticatedSnapshotHarnessV1),
}

enum BoundAuthenticatedSnapshotV1 {
    Production {
        authenticated_confidential_snapshot: Infallible,
    },
    #[cfg(test)]
    TestOnly(TestAuthenticatedSnapshotHarnessV1),
}

#[cfg(test)]
static TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
struct TestAuthenticatedSnapshotHarnessV1 {
    secret_probe: [u8; 32],
    snapshot_digest: [u8; 32],
    commitment_inventory_digest: [u8; 32],
}

#[cfg(test)]
impl Drop for TestAuthenticatedSnapshotHarnessV1 {
    fn drop(&mut self) {
        self.secret_probe.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        debug_assert!(self.secret_probe.iter().all(|byte| *byte == 0));
        TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    }
}

#[path = "vector_arithmetic_plane_openings_v1/global_lookup_proof_session_v3.rs"]
mod global_lookup_proof_session_v3;
#[path = "vector_arithmetic_plane_openings_v1/replay_caps_v1.rs"]
mod replay_caps_v1;
#[path = "vector_arithmetic_plane_openings_v1/vector_arithmetic_proof_codec_v2.rs"]
mod vector_arithmetic_proof_codec_v2;

use replay_caps_v1::PlaneOpeningReplayPermitsV1;

struct PlaneOpeningOwnerLiveV1 {
    snapshot: BoundAuthenticatedSnapshotV1,
    record: PlaneOpeningRecordV1,
    permits: PlaneOpeningReplayPermitsV1,
    completed_replay_digest: [u8; 32],
    completed_replays: u8,
}

#[must_use = "dropping this owner destroys the sole retained opening snapshot"]
struct GlobalLookupPlaneOpeningOwnerV1 {
    live: Option<PlaneOpeningOwnerLiveV1>,
}

struct GlobalLookupPlaneOpeningReplayV1 {
    live: Option<PlaneOpeningOwnerLiveV1>,
    cursor: Option<replay_caps_v1::PlaneOpeningReplayCursorV1>,
}

struct ConsumedGlobalLookupPlaneOpeningOwnerV1 {
    binding_digest: [u8; 32],
}

impl GlobalLookupPlaneOpeningMaterializerSealV1 {
    fn bind_v1(
        self,
        axes: PlaneOpeningSourceContextV1,
    ) -> Result<GlobalLookupPlaneOpeningOwnerV1, PlaneOpeningErrorV1> {
        let context_digest = plane_context_digest_v1(axes)?;
        let mapping_digest = plane_mapping_digest_v1()?;
        let (snapshot, snapshot_digest, commitment_inventory_digest) = match self {
            Self::Production {
                authenticated_confidential_snapshot,
                ..
            } => match authenticated_confidential_snapshot {},
            #[cfg(test)]
            Self::TestOnly(harness) => {
                if harness.snapshot_digest == [0; 32]
                    || harness.commitment_inventory_digest == [0; 32]
                {
                    return Err(PlaneOpeningErrorV1::Source);
                }
                let snapshot_digest = harness.snapshot_digest;
                let commitment_inventory_digest = harness.commitment_inventory_digest;
                (
                    BoundAuthenticatedSnapshotV1::TestOnly(harness),
                    snapshot_digest,
                    commitment_inventory_digest,
                )
            }
        };
        let mut record = PlaneOpeningRecordV1 {
            topology_digest: global_lookup_topology_digest_v1(),
            challenge_manifest_digest: challenge_manifest_digest_v1(),
            basis_digest: ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
            mapping_digest,
            context_digest,
            source_context_axes: axes,
            authenticated_snapshot_digest: snapshot_digest,
            commitment_inventory_digest,
            record_digest: [0; 32],
        };
        record.record_digest = plane_record_digest_v1(&record)?;
        validate_plane_record_v1(&record)?;
        let permits = PlaneOpeningReplayPermitsV1::new_v1(context_digest, mapping_digest)?;
        Ok(GlobalLookupPlaneOpeningOwnerV1 {
            live: Some(PlaneOpeningOwnerLiveV1 {
                snapshot,
                record,
                permits,
                completed_replay_digest: context_digest,
                completed_replays: 0,
            }),
        })
    }

    #[cfg(test)]
    fn test_only_v1(secret_probe: [u8; 32]) -> Self {
        Self::TestOnly(TestAuthenticatedSnapshotHarnessV1 {
            secret_probe,
            snapshot_digest: [0xa5; 32],
            commitment_inventory_digest: [0x5a; 32],
        })
    }
}

impl GlobalLookupPlaneOpeningOwnerV1 {
    fn start_replay_v1(
        mut self,
        purpose: replay_caps_v1::PlaneOpeningReplayPurposeV1,
        expected_context_digest: [u8; 32],
    ) -> Result<GlobalLookupPlaneOpeningReplayV1, PlaneOpeningErrorV1> {
        let mut live = self.live.take().ok_or(PlaneOpeningErrorV1::Replay)?;
        validate_plane_record_v1(&live.record)?;
        if live.record.context_digest != expected_context_digest {
            return Err(PlaneOpeningErrorV1::Context);
        }
        let cursor = live.permits.take_cursor_v1(
            purpose,
            expected_context_digest,
            live.record.mapping_digest,
        )?;
        Ok(GlobalLookupPlaneOpeningReplayV1 {
            live: Some(live),
            cursor: Some(cursor),
        })
    }

    fn finish_v1(mut self) -> Result<ConsumedGlobalLookupPlaneOpeningOwnerV1, PlaneOpeningErrorV1> {
        let live = self.live.take().ok_or(PlaneOpeningErrorV1::Replay)?;
        validate_plane_record_v1(&live.record)?;
        if live.completed_replays != replay_caps_v1::REPLAY_PURPOSE_COUNT_V1 as u8 {
            return Err(PlaneOpeningErrorV1::Replay);
        }
        let permit_digest = live.permits.complete_v1()?;
        let mut hash = Keccak256::new();
        hash.update(PLANE_RECORD_DOMAIN_V1);
        hash.update(b"consumed-owner\0");
        hash.update(&live.record.record_digest);
        hash.update(&live.completed_replay_digest);
        hash.update(&permit_digest);
        let binding_digest = require_nonzero_v1(hash.finalize())?;
        Ok(ConsumedGlobalLookupPlaneOpeningOwnerV1 { binding_digest })
    }
}

impl GlobalLookupPlaneOpeningReplayV1 {
    fn absorb_next_authenticated_plane_v1(
        &mut self,
        ordinal: usize,
    ) -> Result<(), PlaneOpeningErrorV1> {
        self.cursor
            .as_mut()
            .ok_or(PlaneOpeningErrorV1::Replay)?
            .absorb_next_plane_v1(ordinal)
    }

    fn complete_v1(mut self) -> Result<GlobalLookupPlaneOpeningOwnerV1, PlaneOpeningErrorV1> {
        let mut live = self.live.take().ok_or(PlaneOpeningErrorV1::Replay)?;
        let cursor = self.cursor.take().ok_or(PlaneOpeningErrorV1::Replay)?;
        let completion_digest = cursor.complete_v1()?;
        let mut hash = Keccak256::new();
        hash.update(PLANE_RECORD_DOMAIN_V1);
        hash.update(b"completed-replay\0");
        hash.update(&live.completed_replay_digest);
        hash.update(&completion_digest);
        live.completed_replay_digest = require_nonzero_v1(hash.finalize())?;
        live.completed_replays = live
            .completed_replays
            .checked_add(1)
            .ok_or(PlaneOpeningErrorV1::Resource)?;
        Ok(GlobalLookupPlaneOpeningOwnerV1 { live: Some(live) })
    }
}

fn require_nonzero_v1(digest: [u8; 32]) -> Result<[u8; 32], PlaneOpeningErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(PlaneOpeningErrorV1::Context)
}

#[cfg(test)]
#[path = "vector_arithmetic_plane_openings_v1_tests.rs"]
mod tests;
