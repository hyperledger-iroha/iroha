use core::sync::atomic::Ordering;

use super::replay_caps_v1::{
    PlaneOpeningReplayPurposeV1, REPLAY_PURPOSE_COUNT_V1, replay_plane_count_v1,
};
use super::*;

fn source_axes_v1() -> PlaneOpeningSourceContextV1 {
    PlaneOpeningSourceContextV1 {
        source_replay_record_digest: [0x11; 32],
        source_opening_record_digest: [0x22; 32],
        canonical_reopen_record_digest: [0x33; 32],
        radix_range_record_digest: [0x44; 32],
        coefficient_residual_manifest_digest: [0x55; 32],
        committed_mle_profile_digest: [0x66; 32],
    }
}

fn test_owner_v1(secret_probe: [u8; 32]) -> (GlobalLookupPlaneOpeningOwnerV1, [u8; 32]) {
    let axes = source_axes_v1();
    let context = plane_context_digest_v1(axes).expect("test context");
    let owner = GlobalLookupPlaneOpeningMaterializerSealV1::test_only_v1(secret_probe)
        .bind_v1(axes)
        .expect("test owner");
    (owner, context)
}

fn complete_purpose_v1(
    owner: GlobalLookupPlaneOpeningOwnerV1,
    context: [u8; 32],
    purpose: PlaneOpeningReplayPurposeV1,
) -> GlobalLookupPlaneOpeningOwnerV1 {
    let mut replay = owner
        .start_replay_v1(purpose, context)
        .expect("purpose-bound replay");
    for ordinal in 0..PLANE_COUNT_V1 {
        let coordinate = plane_coordinate_v1(ordinal).expect("plane coordinate");
        if purpose.accepts_role_v1(coordinate.role) {
            replay
                .absorb_next_authenticated_plane_v1(ordinal)
                .expect("canonical replay plane");
        }
    }
    replay.complete_v1().expect("complete replay")
}

#[test]
fn exact_plane_roles_ranges_and_axes_are_frozen() {
    let mut role_counts = [0_usize; 9];
    for ordinal in 0..PLANE_COUNT_V1 {
        let coordinate = plane_coordinate_v1(ordinal).unwrap();
        assert_eq!(usize::from(coordinate.ordinal), ordinal);
        role_counts[coordinate.role as usize - 1] += 1;
        match coordinate.role {
            GlobalLookupPlaneRoleV1::BooleanD
            | GlobalLookupPlaneRoleV1::BooleanS
            | GlobalLookupPlaneRoleV1::MixedTop => {
                assert!(coordinate.group.is_some());
                assert_eq!(
                    (coordinate.unit, coordinate.column, coordinate.statement),
                    (None, None, None)
                );
            }
            GlobalLookupPlaneRoleV1::ComparatorBorrow => {
                assert!(coordinate.group.is_some() && coordinate.column.is_some());
                assert_eq!((coordinate.unit, coordinate.statement), (None, None));
            }
            GlobalLookupPlaneRoleV1::SmallSigned
            | GlobalLookupPlaneRoleV1::SmallNegativeMagnitude => {
                assert!(coordinate.unit.is_some());
                assert_eq!(
                    (coordinate.group, coordinate.column, coordinate.statement),
                    (None, None, None)
                );
            }
            GlobalLookupPlaneRoleV1::ResidualQ3
            | GlobalLookupPlaneRoleV1::ResidualQ5
            | GlobalLookupPlaneRoleV1::ResidualQ8 => {
                assert!(coordinate.statement.is_some());
                assert_eq!(
                    (coordinate.group, coordinate.unit, coordinate.column),
                    (None, None, None)
                );
            }
        }
    }
    assert_eq!(role_counts, [344, 344, 6_192, 344, 1_032, 1_032, 1, 1, 1]);
    for role in [
        GlobalLookupPlaneRoleV1::BooleanD,
        GlobalLookupPlaneRoleV1::BooleanS,
        GlobalLookupPlaneRoleV1::ComparatorBorrow,
        GlobalLookupPlaneRoleV1::MixedTop,
        GlobalLookupPlaneRoleV1::SmallSigned,
        GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
        GlobalLookupPlaneRoleV1::ResidualQ3,
        GlobalLookupPlaneRoleV1::ResidualQ5,
        GlobalLookupPlaneRoleV1::ResidualQ8,
    ] {
        assert_eq!(role_counts[role as usize - 1], role_plane_count_v1(role));
    }
    assert_eq!(
        plane_coordinate_v1(PLANE_COUNT_V1),
        Err(PlaneOpeningErrorV1::Shape)
    );
}

#[test]
fn group_major_beta_unit_and_q_boundaries_are_exact() {
    for (ordinal, role, group, unit, column, statement) in [
        (
            0,
            GlobalLookupPlaneRoleV1::BooleanD,
            Some(0),
            None,
            None,
            None,
        ),
        (
            343,
            GlobalLookupPlaneRoleV1::BooleanD,
            Some(343),
            None,
            None,
            None,
        ),
        (
            344,
            GlobalLookupPlaneRoleV1::BooleanS,
            Some(0),
            None,
            None,
            None,
        ),
        (
            687,
            GlobalLookupPlaneRoleV1::BooleanS,
            Some(343),
            None,
            None,
            None,
        ),
        (
            688,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(0),
            None,
            Some(0),
            None,
        ),
        (
            705,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(0),
            None,
            Some(17),
            None,
        ),
        (
            706,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(1),
            None,
            Some(0),
            None,
        ),
        (
            6_879,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(343),
            None,
            Some(17),
            None,
        ),
        (
            6_880,
            GlobalLookupPlaneRoleV1::MixedTop,
            Some(0),
            None,
            None,
            None,
        ),
        (
            7_224,
            GlobalLookupPlaneRoleV1::SmallSigned,
            None,
            Some(0),
            None,
            None,
        ),
        (
            8_255,
            GlobalLookupPlaneRoleV1::SmallSigned,
            None,
            Some(1_031),
            None,
            None,
        ),
        (
            8_256,
            GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
            None,
            Some(0),
            None,
            None,
        ),
        (
            9_287,
            GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
            None,
            Some(1_031),
            None,
            None,
        ),
        (
            9_288,
            GlobalLookupPlaneRoleV1::ResidualQ3,
            None,
            None,
            None,
            Some(3),
        ),
        (
            9_289,
            GlobalLookupPlaneRoleV1::ResidualQ5,
            None,
            None,
            None,
            Some(5),
        ),
        (
            9_290,
            GlobalLookupPlaneRoleV1::ResidualQ8,
            None,
            None,
            None,
            Some(8),
        ),
    ] {
        let coordinate = plane_coordinate_v1(ordinal).unwrap();
        assert_eq!(
            (
                coordinate.role,
                coordinate.group.map(usize::from),
                coordinate.unit.map(usize::from),
                coordinate.column.map(usize::from),
                coordinate.statement
            ),
            (role, group, unit, column, statement)
        );
    }
    for literal in [
        b"group=record*8+group-in-record".as_slice(),
        b"unit=((record*3+signed-role)*8+plane)",
        b"signed-role=(r,e0,e1)",
        b"beta-order=group-major-then-column",
        b"Boolean-coordinate-bits-little-endian",
        b"plane-order=bD[group],bS[group],beta[group][column],m[group],x[unit],n[unit],q3,q5,q8",
    ] {
        let schemas = [
            GROUP_AXIS_LANGUAGE_V1,
            UNIT_AXIS_LANGUAGE_V1,
            COLUMN_AXIS_LANGUAGE_V1,
            COORDINATE_AXIS_LANGUAGE_V1,
            PLANE_ORDER_LANGUAGE_V1,
        ]
        .concat();
        assert!(
            schemas
                .windows(literal.len())
                .any(|window| window == literal)
        );
    }
}

#[test]
fn one_snapshot_geometry_exposes_the_current_honest_blocker() {
    assert_eq!(PLANE_COUNT_V1, 9_291);
    assert_eq!(SNAPSHOT_SLOTS_PER_PLANE_V1, 33);
    assert_eq!(SNAPSHOT_SLOT_COUNT_V1, 306_603);
    assert_eq!(RETAINED_VALUE_BYTES_V1, 4_871_159_808);
    assert_eq!(RETAINED_BLINDING_BYTES_V1, 297_312);
    assert_eq!(RETAINED_COMMITMENT_WIRE_BYTES_V1, 306_603);
    assert_eq!(SNAPSHOT_SEMANTIC_BYTES_V1, 4_871_763_723);
    assert_eq!(SNAPSHOT_ZERO_PADDING_BYTES_V1, 151_619_829);
    assert_eq!(SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1, 5_023_383_552);
    assert_eq!(SNAPSHOT_AUTHENTICATION_TAG_BYTES_V1, 4_905_648);
    assert_eq!(SNAPSHOT_FILE_BYTES_V1, 5_028_289_200);
    assert!(SNAPSHOT_SLOT_COUNT_V1 <= CONFIDENTIAL_SPOOL_MAX_SLOTS_V1);
    assert!(SNAPSHOT_FILE_BYTES_V1 > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!(
        SNAPSHOT_LAYOUT_LANGUAGE_V1
            .windows(b"one-authenticated-confidential-snapshot".len())
            .any(|window| window == b"one-authenticated-confidential-snapshot")
    );
    assert!(
        SNAPSHOT_LAYOUT_LANGUAGE_V1
            .windows(b"blinding32||nonidentity-commitment33".len())
            .any(|window| window == b"blinding32||nonidentity-commitment33")
    );
    assert!(
        !SNAPSHOT_LAYOUT_LANGUAGE_V1
            .windows(b"shard".len())
            .any(|window| window == b"shard")
    );
    assert!(!CURRENT_UPSTREAM_COMPLETE_V1 && !CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1);
}

#[test]
fn source_context_is_ordered_nonzero_and_swap_hostile() {
    let axes = source_axes_v1();
    let digest = plane_context_digest_v1(axes).unwrap();
    assert_eq!(digest, plane_context_digest_v1(axes).unwrap());
    assert_ne!(digest, plane_mapping_digest_v1().unwrap());

    let mut swapped = axes;
    core::mem::swap(
        &mut swapped.source_replay_record_digest,
        &mut swapped.source_opening_record_digest,
    );
    assert_ne!(digest, plane_context_digest_v1(swapped).unwrap());

    let mut zero = axes;
    zero.canonical_reopen_record_digest = [0; 32];
    assert_eq!(
        plane_context_digest_v1(zero),
        Err(PlaneOpeningErrorV1::Context)
    );
    for literal in [
        b"topology,challenge-manifest,basis,mapping".as_slice(),
        b"source-replay-record,source-opening-record,canonical-reopen-record",
        b"coefficient-residual-manifest,committed-MLE-profile",
    ] {
        assert!(
            SOURCE_CONTEXT_LANGUAGE_V1
                .windows(literal.len())
                .any(|window| window == literal)
        );
    }
}

#[test]
fn exact_replay_purposes_authorize_only_required_multi_use() {
    let purposes = [
        PlaneOpeningReplayPurposeV1::Statement3DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement3CoefficientIpa,
        PlaneOpeningReplayPurposeV1::Statement5DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement5CoefficientIpa,
        PlaneOpeningReplayPurposeV1::Statement8DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement8CoefficientIpa,
    ];
    let counts = purposes.map(|purpose| replay_plane_count_v1(purpose).unwrap());
    assert_eq!(counts, [689, 1, 6_881, 1, 2_065, 1]);
    assert_eq!(counts.into_iter().sum::<usize>(), 9_638);
    assert_eq!(REPLAY_PURPOSE_COUNT_V1, 6);

    for role in [
        GlobalLookupPlaneRoleV1::BooleanD,
        GlobalLookupPlaneRoleV1::BooleanS,
        GlobalLookupPlaneRoleV1::ComparatorBorrow,
        GlobalLookupPlaneRoleV1::MixedTop,
        GlobalLookupPlaneRoleV1::SmallSigned,
        GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
        GlobalLookupPlaneRoleV1::ResidualQ3,
        GlobalLookupPlaneRoleV1::ResidualQ5,
        GlobalLookupPlaneRoleV1::ResidualQ8,
    ] {
        let uses = purposes
            .iter()
            .filter(|purpose| purpose.accepts_role_v1(role))
            .count();
        let expected = usize::from(matches!(
            role,
            GlobalLookupPlaneRoleV1::BooleanD
                | GlobalLookupPlaneRoleV1::ResidualQ3
                | GlobalLookupPlaneRoleV1::ResidualQ5
                | GlobalLookupPlaneRoleV1::ResidualQ8
        )) + 1;
        assert_eq!(uses, expected);
    }
}

#[test]
fn every_permit_is_one_shot_and_full_consumption_releases_no_authority() {
    let before = TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst);
    let (mut owner, context) = test_owner_v1([0x7a; 32]);
    for purpose in [
        PlaneOpeningReplayPurposeV1::Statement5DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement3CoefficientIpa,
        PlaneOpeningReplayPurposeV1::Statement8DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement3DerivedLro,
        PlaneOpeningReplayPurposeV1::Statement8CoefficientIpa,
        PlaneOpeningReplayPurposeV1::Statement5CoefficientIpa,
    ] {
        owner = complete_purpose_v1(owner, context, purpose);
    }
    let consumed = owner.finish_v1().unwrap();
    assert_ne!(consumed.binding_digest, [0; 32]);
    assert!(TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst) >= before + 1);
    assert!(!AUTHORITY_MINTED_V1 && !RELEASE_READY_V1);
}

#[test]
fn wrong_order_context_duplicate_and_incomplete_replays_fail_closed() {
    let before = TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst);
    let (owner, context) = test_owner_v1([0x81; 32]);
    let mut replay = owner
        .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement3DerivedLro, context)
        .unwrap();
    assert_eq!(
        replay.absorb_next_authenticated_plane_v1(1),
        Err(PlaneOpeningErrorV1::Order)
    );
    assert_eq!(
        replay.absorb_next_authenticated_plane_v1(0),
        Err(PlaneOpeningErrorV1::Replay)
    );
    assert!(replay.complete_v1().is_err());

    let (owner, _) = test_owner_v1([0x82; 32]);
    assert!(
        owner
            .start_replay_v1(
                PlaneOpeningReplayPurposeV1::Statement3CoefficientIpa,
                [0xff; 32]
            )
            .is_err()
    );

    let (owner, context) = test_owner_v1([0x83; 32]);
    let owner = complete_purpose_v1(
        owner,
        context,
        PlaneOpeningReplayPurposeV1::Statement3CoefficientIpa,
    );
    assert!(
        owner
            .start_replay_v1(
                PlaneOpeningReplayPurposeV1::Statement3CoefficientIpa,
                context
            )
            .is_err()
    );

    let (owner, _) = test_owner_v1([0x84; 32]);
    assert!(owner.finish_v1().is_err());
    assert!(TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst) >= before + 4);
}

#[test]
fn owner_zeroizes_on_unwind_and_record_context_tampering() {
    let before = TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (_owner, _context) = test_owner_v1([0x91; 32]);
        panic!("intentional retained-opening owner unwind");
    }));
    assert!(result.is_err());

    let (mut owner, context) = test_owner_v1([0x92; 32]);
    owner.live.as_mut().unwrap().record.context_digest = [0x93; 32];
    assert!(
        owner
            .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement8DerivedLro, context)
            .is_err()
    );
    assert!(TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst) >= before + 2);
}

#[test]
fn production_source_and_release_guards_are_static() {
    let production = include_str!("vector_arithmetic_plane_openings_v1.rs");
    let caps = include_str!("vector_arithmetic_plane_openings_v1/replay_caps_v1.rs");
    let parent = include_str!("../global_lookup_statement_v1.rs");
    assert!(production.lines().count() <= 750);
    assert!(caps.lines().count() <= 400);
    assert!(
        include_str!("vector_arithmetic_plane_openings_v1_tests.rs")
            .lines()
            .count()
            <= 600
    );
    assert_eq!(
        parent
            .matches("mod vector_arithmetic_plane_openings_v1;")
            .count(),
        1
    );
    let topology_body = parent
        .split("pub(super) fn global_lookup_topology_digest_v1()")
        .nth(1)
        .unwrap()
        .split("const fn endpoint_tag_v1")
        .next()
        .unwrap();
    assert!(!topology_body.contains("vector_arithmetic_plane_openings_v1"));
    assert!(!production.contains("std::path"));
    assert!(!production.contains("PathBuf"));
    assert!(!production.contains("ConfidentialSpoolSnapshotV1"));
    assert!(!production.contains("fn raw_"));
    assert!(!production.contains("fn as_slice"));
    assert!(!production.contains("impl Clone for GlobalLookupPlaneOpeningOwnerV1"));
    assert!(!production.contains("derive(Clone)]\nstruct GlobalLookupPlaneOpeningOwnerV1"));
    assert!(!caps.contains("derive(Clone)]\nstruct PlaneOpeningReplayPermitV1"));
    for field in [
        "authenticated_confidential_snapshot: Infallible",
        "exact_plane_values: Infallible",
        "exact_commitment_blindings: Infallible",
        "exact_commitment_inventory: Infallible",
        "authenticated_source_context: Infallible",
    ] {
        assert!(production.contains(field));
    }
    assert_eq!((TRANSCRIPT_FRAMES_ADDED_V1, WIRE_BYTES_ADDED_V1), (0, 0));
    assert!(!KAT_ORDINALS_CHANGED_V1);
    for gate in [
        CURRENT_UPSTREAM_COMPLETE_V1,
        CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1,
        PLANE_OPENING_MATERIALIZED_V1,
        VECTOR_ARITHMETIC_PROOFS_WIRED_V1,
        VECTOR_ARITHMETIC_PROOFS_VERIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        OPERATIONAL_RECEIPT_ACCEPTED_V1,
        AUTHORITY_MINTED_V1,
        RSS_QUALIFIED_V1,
        RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
}
