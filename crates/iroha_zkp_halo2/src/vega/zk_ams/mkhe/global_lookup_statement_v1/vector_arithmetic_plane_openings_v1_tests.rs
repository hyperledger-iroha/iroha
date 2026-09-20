//! Current native40 plane geometry, custody and replay failure controls.
use core::sync::atomic::Ordering;

use iroha_crypto::confidential_spool::ConfidentialSpoolLayoutV1;

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
    let mut counts = [0_usize; 6];
    for ordinal in 0..PLANE_COUNT_V1 {
        let c = plane_coordinate_v1(ordinal).unwrap();
        assert_eq!(usize::from(c.ordinal), ordinal);
        counts[c.role as usize - 1] += 1;
        match c.role {
            GlobalLookupPlaneRoleV1::BooleanD
            | GlobalLookupPlaneRoleV1::BooleanS
            | GlobalLookupPlaneRoleV1::MixedTop => {
                assert!(c.group.is_some());
                assert_eq!((c.unit, c.column), (None, None));
            }
            GlobalLookupPlaneRoleV1::ComparatorBorrow => {
                assert!(c.group.is_some() && c.column.is_some());
                assert_eq!(c.unit, None);
            }
            GlobalLookupPlaneRoleV1::SmallSigned
            | GlobalLookupPlaneRoleV1::SmallNegativeMagnitude => {
                assert!(c.unit.is_some());
                assert_eq!((c.group, c.column), (None, None));
            }
        }
    }
    assert_eq!(counts, [344, 344, 6192, 344, 1032, 1032]);
    for role in [
        GlobalLookupPlaneRoleV1::BooleanD,
        GlobalLookupPlaneRoleV1::BooleanS,
        GlobalLookupPlaneRoleV1::ComparatorBorrow,
        GlobalLookupPlaneRoleV1::MixedTop,
        GlobalLookupPlaneRoleV1::SmallSigned,
        GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
    ] {
        assert_eq!(counts[role as usize - 1], role_plane_count_v1(role));
    }
    for retired in [9288, 9289, 9290, usize::MAX] {
        assert_eq!(
            plane_coordinate_v1(retired),
            Err(PlaneOpeningErrorV1::Shape)
        );
    }
}

#[test]
fn group_major_beta_and_unit_boundaries_are_exact() {
    for (ordinal, role, group, unit, column) in [
        (0, GlobalLookupPlaneRoleV1::BooleanD, Some(0), None, None),
        (
            343,
            GlobalLookupPlaneRoleV1::BooleanD,
            Some(343),
            None,
            None,
        ),
        (344, GlobalLookupPlaneRoleV1::BooleanS, Some(0), None, None),
        (
            687,
            GlobalLookupPlaneRoleV1::BooleanS,
            Some(343),
            None,
            None,
        ),
        (
            688,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(0),
            None,
            Some(0),
        ),
        (
            705,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(0),
            None,
            Some(17),
        ),
        (
            706,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(1),
            None,
            Some(0),
        ),
        (
            6879,
            GlobalLookupPlaneRoleV1::ComparatorBorrow,
            Some(343),
            None,
            Some(17),
        ),
        (6880, GlobalLookupPlaneRoleV1::MixedTop, Some(0), None, None),
        (
            7223,
            GlobalLookupPlaneRoleV1::MixedTop,
            Some(343),
            None,
            None,
        ),
        (
            7224,
            GlobalLookupPlaneRoleV1::SmallSigned,
            None,
            Some(0),
            None,
        ),
        (
            8255,
            GlobalLookupPlaneRoleV1::SmallSigned,
            None,
            Some(1031),
            None,
        ),
        (
            8256,
            GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
            None,
            Some(0),
            None,
        ),
        (
            9287,
            GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
            None,
            Some(1031),
            None,
        ),
    ] {
        let c = plane_coordinate_v1(ordinal).unwrap();
        assert_eq!(
            (
                c.role,
                c.group.map(usize::from),
                c.unit.map(usize::from),
                c.column.map(usize::from)
            ),
            (role, group, unit, column)
        );
    }
    for literal in [
        b"group=record*8+group-in-record".as_slice(),
        b"unit=((record*3+signed-role)*8+plane)",
        b"signed-role=(r,e0,e1)",
        b"beta-order=group-major-then-column",
        b"Boolean-coordinate-bits-little-endian",
        b"plane-order=bD[group],bS[group],beta[group][column],m[group],x[unit],n[unit]",
    ] {
        let schemas = [
            GROUP_AXIS_LANGUAGE_V1,
            UNIT_AXIS_LANGUAGE_V1,
            COLUMN_AXIS_LANGUAGE_V1,
            COORDINATE_AXIS_LANGUAGE_V1,
            PLANE_ORDER_LANGUAGE_V1,
        ]
        .concat();
        assert!(schemas.windows(literal.len()).any(|w| w == literal));
    }
}

#[test]
fn ordered_plan_fits_without_changing_the_exact_single_file_cap_deficit() {
    assert_eq!(PLANE_COUNT_V1, 9_288);
    assert_eq!(COMMITMENT_MASKS_V1, 9_288);
    assert_eq!(SNAPSHOT_SLOTS_PER_PLANE_V1, 33);
    assert_eq!(SNAPSHOT_SLOT_COUNT_V1, 306_504);
    assert_eq!(RETAINED_VALUE_BYTES_V1, 4_869_586_944);
    assert_eq!(RETAINED_BLINDING_BYTES_V1, 297_216);
    assert_eq!(RETAINED_COMMITMENT_WIRE_BYTES_V1, 306_504);
    assert_eq!(SNAPSHOT_SEMANTIC_BYTES_V1, 4_870_190_664);
    assert_eq!(SNAPSHOT_ZERO_PADDING_BYTES_V1, 151_570_872);
    assert_eq!(SNAPSHOT_PADDED_PLAINTEXT_BYTES_V1, 5_021_761_536);
    assert_eq!(SNAPSHOT_AUTHENTICATION_TAG_BYTES_V1, 4_904_064);
    assert_eq!(SNAPSHOT_FILE_BYTES_V1, 5_026_665_600);
    assert_eq!(SNAPSHOT_GENERAL_FILE_CAP_EXCESS_BYTES_V1, 1_197_141_120);
    assert!(SNAPSHOT_SLOT_COUNT_V1 <= CONFIDENTIAL_SPOOL_MAX_SLOTS_V1);
    assert!(SNAPSHOT_FILE_BYTES_V1 > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1);
    assert!(SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1 <= CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1);
    let context = plane_context_digest_v1(source_axes_v1()).unwrap();
    assert_eq!(
        ConfidentialSpoolLayoutV1::new_v1(
            SNAPSHOT_SLOT_COUNT_V1,
            SNAPSHOT_SLOT_PLAINTEXT_BYTES_V1,
            context,
        ),
        Err(
            iroha_crypto::confidential_spool::ConfidentialSpoolErrorV1::LimitExceeded(
                "file length"
            )
        )
    );
    let plan = approved_snapshot_plan_v1(context).expect("ordered full geometry");
    assert_eq!(plan.slot_count_v1(), SNAPSHOT_SLOT_COUNT_V1);
    assert_eq!(
        hex::encode(plan.descriptor_digest_v1()),
        "d2ed7749c88a42e46acdf66882b1d3f5aa8af9851ec2df93ac14f853fbd8e092"
    );
    assert_eq!(
        plan,
        ordered_snapshot_v1::OrderedPlaneSpoolPlanV1::canonical_v1(context).unwrap()
    );
    assert_eq!(
        approved_snapshot_plan_v1([0; 32]),
        Err(PlaneOpeningErrorV1::Context)
    );
    assert!(
        SNAPSHOT_LAYOUT_LANGUAGE_V1
            .windows(b"one-authenticated-confidential-snapshot".len())
            .any(|window| window == b"one-authenticated-confidential-snapshot")
    );
    assert!(
        COMMITMENT_LANGUAGE_V1
            .windows(b"commitment-mask[plane]=blinding[plane];mask-order=plane-order".len())
            .any(|window| {
                window == b"commitment-mask[plane]=blinding[plane];mask-order=plane-order"
            })
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
fn record_rejects_a_substituted_ordered_plan_even_with_a_rehashed_record() {
    let (mut owner, context) = test_owner_v1([0x95; 32]);
    let record = &mut owner.live.as_mut().unwrap().record;
    let expected = approved_snapshot_plan_v1(context)
        .unwrap()
        .descriptor_digest_v1();
    assert_eq!(record.ordered_snapshot_plan_digest, expected);
    for digest in [
        [0; 32],
        approved_snapshot_plan_v1([0x96; 32])
            .unwrap()
            .descriptor_digest_v1(),
    ] {
        record.ordered_snapshot_plan_digest = digest;
        if digest != [0; 32] {
            record.record_digest = plane_record_digest_v1(record).unwrap();
        }
        assert_eq!(
            validate_plane_record_v1(record),
            Err(PlaneOpeningErrorV1::Context)
        );
    }
    record.ordered_snapshot_plan_digest = expected;
    record.record_digest = plane_record_digest_v1(record).unwrap();
    validate_plane_record_v1(record).unwrap();
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
        b"native40-inventory,basis,mapping".as_slice(),
        b"source-replay-record,source-opening-record,canonical-reopen-record",
        b"radix-range-record",
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
        PlaneOpeningReplayPurposeV1::Statement3Inputs,
        PlaneOpeningReplayPurposeV1::Statement5Inputs,
        PlaneOpeningReplayPurposeV1::Statement8Inputs,
    ];
    let counts = purposes.map(|p| replay_plane_count_v1(p).unwrap());
    assert_eq!(counts, [688, 6880, 2064]);
    assert_eq!(counts.into_iter().sum::<usize>(), 9632);
    assert_eq!(REPLAY_PURPOSE_COUNT_V1, 3);
    for role in [
        GlobalLookupPlaneRoleV1::BooleanD,
        GlobalLookupPlaneRoleV1::BooleanS,
        GlobalLookupPlaneRoleV1::ComparatorBorrow,
        GlobalLookupPlaneRoleV1::MixedTop,
        GlobalLookupPlaneRoleV1::SmallSigned,
        GlobalLookupPlaneRoleV1::SmallNegativeMagnitude,
    ] {
        let uses = purposes.iter().filter(|p| p.accepts_role_v1(role)).count();
        assert_eq!(
            uses,
            usize::from(role == GlobalLookupPlaneRoleV1::BooleanD) + 1
        );
    }
}

#[test]
fn every_permit_is_one_shot_and_full_consumption_releases_no_authority() {
    let before = TEST_ZEROIZED_SNAPSHOT_HARNESSES_V1.load(Ordering::SeqCst);
    let (mut owner, context) = test_owner_v1([0x7a; 32]);
    for purpose in [
        PlaneOpeningReplayPurposeV1::Statement5Inputs,
        PlaneOpeningReplayPurposeV1::Statement8Inputs,
        PlaneOpeningReplayPurposeV1::Statement3Inputs,
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
        .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement3Inputs, context)
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
            .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement3Inputs, [0xff; 32])
            .is_err()
    );

    let (owner, context) = test_owner_v1([0x83; 32]);
    let owner = complete_purpose_v1(
        owner,
        context,
        PlaneOpeningReplayPurposeV1::Statement3Inputs,
    );
    assert!(
        owner
            .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement3Inputs, context)
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
            .start_replay_v1(PlaneOpeningReplayPurposeV1::Statement8Inputs, context)
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
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(!topology_body.contains("vector_arithmetic_plane_openings_v1"));
    let registration = "#[path = \"vector_arithmetic_plane_openings_v1/ordered_snapshot_v1.rs\"]\nmod ordered_snapshot_v1;";
    assert_eq!(production.matches(registration).count(), 1);
    assert!(!production.contains(&format!("#[cfg(test)]\n{registration}")));
    assert!(!production.contains("approved_snapshot_layout_v1"));
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
    for gate in [
        CURRENT_UPSTREAM_COMPLETE_V1,
        PLANE_OPENING_MATERIALIZED_V1,
        DIRECT_PRODUCT_SOURCE_REPLAYS_WIRED_V1,
        COMPLETE_OPENING_EQUATIONS_VERIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        OPERATIONAL_RECEIPT_ACCEPTED_V1,
        AUTHORITY_MINTED_V1,
        RSS_QUALIFIED_V1,
        RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
    assert!(!CURRENT_SINGLE_SNAPSHOT_BACKEND_FITS_V1);
}
