//! Canonical source-session inventory and entropy/custody rejection controls.
use super::*;
use std::panic::AssertUnwindSafe;

const SESSION_SOURCE_V1: &str = include_str!("commitment_session_v1.rs");
const SESSION_TEST_SOURCE_V1: &str = include_str!("commitment_session_v1_tests.rs");
const OPENING_SOURCE_V1: &str = include_str!("../source_openings_v1.rs");
const REPLAY_SOURCE_V1: &str = include_str!("../../global_lookup_source_replay_v1.rs");
const RADIX_SOURCE_V2: &str = include_str!("../../../incremental_source_phase23_radix_range_v2.rs");

fn reference_source_blinding_bytes_v1(seed: [u8; 32], group: u16) -> [u8; 32] {
    for attempt in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut encoded = [0_u8; 32];
        let mut hash = Keccak256::new();
        hash.update(TEST_ENTROPY_DOMAIN_V1);
        hash.update(&seed);
        hash.update(&group.to_be_bytes());
        hash.update(&(attempt as u16).to_be_bytes());
        hash.finalize_into(&mut encoded);
        if let Ok(scalar) = Scalar::from_be_bytes_exact_ref(&encoded) {
            if !scalar.is_zero() {
                return encoded;
            }
        }
    }
    panic!("independent deterministic source fixture exhausted rejection bound");
}

#[test]
fn native40_role_manifest_is_contiguous_complete_and_source_backed() {
    let mut next = 0_u32;
    let mut pre_z = 0_u32;
    let mut post_z = 0_u32;
    for purpose in ALL_POINT_PURPOSES_V1 {
        let count = purpose.count_v1() as u32;
        assert!(count > 0);
        for local in 0..count {
            let coordinate = commitment_coordinate_v1(next + local).unwrap();
            assert_eq!(coordinate.global_ordinal, next + local);
            assert_eq!(coordinate.phase, purpose.phase_v1());
            assert_eq!(coordinate.purpose, purpose);
            assert_eq!(coordinate.purpose_ordinal, local);
        }
        match purpose.phase_v1() {
            GlobalLookupCommitmentPhaseV1::ChallengeIndependent => pre_z += count,
            GlobalLookupCommitmentPhaseV1::PostZ => post_z += count,
        }
        next += count;
    }
    assert_eq!((pre_z, post_z, next), (39_978, 32_408, 72_386));
    assert_eq!(GlobalLookupCommitmentPurposeV1::Source.count_v1(), 344);
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::QMaskDigit.count_v1(),
        6_400
    );
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::QMaskComplementDigit.count_v1(),
        6_400
    );
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::QMaskDigitInverse.count_v1(),
        6_400
    );
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::QMaskComplementInverse.count_v1(),
        6_400
    );
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::InverseProductMask as u8,
        14
    );
    assert!(commitment_coordinate_v1(72_386).is_err());
    assert!(commitment_coordinate_v1(82_804).is_err());
    assert!(commitment_coordinate_v1(u32::MAX).is_err());
}

#[test]
fn comparator_signed_aliases_are_exact_unique_and_exclude_all_inverses() {
    let count: usize = COMPARATOR_SIGNED_POINT_PURPOSES_V1
        .iter()
        .map(|p| p.count_v1())
        .sum();
    assert_eq!(count, 9_288);
    let mut seen = vec![false; GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as usize];
    for ordinal in 0..count as u32 {
        let coordinate = comparator_signed_coordinate_v1(ordinal).unwrap();
        assert!(!seen[coordinate.global_ordinal as usize]);
        seen[coordinate.global_ordinal as usize] = true;
        assert_eq!(
            coordinate.phase,
            GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        );
        assert!(COMPARATOR_SIGNED_POINT_PURPOSES_V1.contains(&coordinate.purpose));
    }
    for (logical, physical) in [
        (0, 12_040),
        (344, 12_384),
        (688, 18_576),
        (6_880, 24_768),
        (7_224, 25_112),
        (8_256, 26_144),
    ] {
        assert_eq!(
            comparator_signed_coordinate_v1(logical)
                .unwrap()
                .global_ordinal,
            physical
        );
    }
    for retired in [9_288, 9_289, 9_290, u32::MAX] {
        assert!(comparator_signed_coordinate_v1(retired).is_err());
    }
    assert!(!seen[39_978..].contains(&true));
}

#[test]
fn exact_storage_and_zero_new_skeleton_io_are_frozen() {
    assert_eq!(INVENTORY_BLINDING_BYTES_V1, 72_386 * 32);
    assert_eq!(INVENTORY_POINT_WIRE_BYTES_V1, 72_386 * 33);
    assert_eq!(INVENTORY_SEMANTIC_BYTES_V1, 4_705_090);
    assert_eq!(INVENTORY_AUTHENTICATION_TAG_BYTES_V1, 72_386 * 16);
    assert_eq!(PROJECTED_INVENTORY_FILE_BYTES_V1, 5_863_266);
    assert_eq!(PROJECTED_INVENTORY_WRITE_AND_SEAL_READ_BYTES_V1, 11_726_532);
    assert_eq!(INVENTORY_SKELETON_NEW_FILE_BYTES_V1, 0);
    assert_eq!(INVENTORY_SKELETON_NEW_IO_BYTES_V1, 0);
    assert_eq!(
        INVENTORY_SKELETON_NAMED_HEAP_BYTES_V1,
        72_386 * core::mem::size_of::<Option<GlobalLookupCommitmentTicketV1>>()
    );
    assert!(!COMPLETE_INVENTORY_MATERIALIZED_V1);
    assert!(!PROOF_ACCOUNTING_QUALIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!AUTHORITY_ACCEPTED_V1);
    assert!(!RSS_QUALIFIED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
    let inventory = GlobalLookupCommitmentInventorySkeletonV1::new_v1().unwrap();
    assert_eq!(inventory.slots.len(), 72_386);
    assert_eq!(inventory.slots.capacity(), 72_386);
    assert!(inventory.slots.iter().all(Option::is_none));
}

#[test]
fn session_is_context_bound_monotonic_and_carries_authenticated_source_adoption() {
    let proof_context = [0x31; 32];
    let opening_context = [0x32; 32];
    let seed = [0x41; 32];
    let generator = Point::canonical_generator().unwrap();
    let mut session =
        GlobalLookupProofSessionEntropySealV1::test_only_v1(proof_context, seed).unwrap();
    session
        .bind_source_opening_context_v1(opening_context)
        .unwrap();
    assert!(session.sample_source_blinding_v1(1).is_err());

    let mut session =
        GlobalLookupProofSessionEntropySealV1::test_only_v1(proof_context, seed).unwrap();
    session
        .bind_source_opening_context_v1(opening_context)
        .unwrap();
    for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 as u32 {
        let (mut chunk, scalar) = session.sample_source_blinding_v1(ordinal).unwrap();
        assert_eq!(chunk.as_mut_slice_v1(), scalar.get().to_be_bytes());
        assert_eq!(
            chunk.as_mut_slice_v1(),
            reference_source_blinding_bytes_v1(seed, ordinal as u16)
        );
        assert!(!scalar.get().is_zero());
        session
            .adopt_source_commitment_v1(ordinal, &generator)
            .unwrap();
    }
    let commitments_root = session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .adopted_source_commitments_root_v1(opening_context)
        .unwrap();
    let mut complete = session
        .complete_source_opening_v1([0x53; 32], opening_context, commitments_root, [0x52; 32])
        .unwrap();
    complete
        .validate_completed_source_prefix_v1(
            [0x53; 32],
            opening_context,
            commitments_root,
            [0x52; 32],
        )
        .unwrap();
    assert!(
        complete
            .validate_completed_source_prefix_v1(
                [0x53; 32],
                [0x33; 32],
                commitments_root,
                [0x52; 32]
            )
            .is_err()
    );
    let live = complete.live.as_ref().unwrap();
    assert_eq!(live.next_global_ordinal, 344);
    assert!(live.inventory.slots[..344].iter().all(Option::is_some));
    assert!(live.inventory.slots[344..].iter().all(Option::is_none));
    for (ordinal, ticket) in live.inventory.slots[..344].iter().enumerate() {
        let ticket = ticket.as_ref().unwrap();
        assert_eq!(ticket.coordinate.global_ordinal, ordinal as u32);
        assert_eq!(
            ticket.point_wire,
            generator.to_non_identity_wire_bytes().unwrap()
        );
    }
    complete.live.as_mut().unwrap().inventory.slots[0]
        .as_mut()
        .unwrap()
        .point_wire[0] ^= 1;
    assert!(
        complete
            .validate_completed_source_prefix_v1(
                [0x53; 32],
                opening_context,
                commitments_root,
                [0x52; 32]
            )
            .is_err()
    );
}

#[test]
fn duplicate_skip_entropy_failure_and_unwind_poison_the_move_only_session() {
    let generator = Point::canonical_generator().unwrap();
    let mut duplicate =
        GlobalLookupProofSessionEntropySealV1::test_only_v1([1; 32], [2; 32]).unwrap();
    duplicate.bind_source_opening_context_v1([3; 32]).unwrap();
    let _pending = duplicate.sample_source_blinding_v1(0).unwrap();
    assert!(duplicate.sample_source_blinding_v1(0).is_err());
    assert!(duplicate.live.is_none());

    let mut wrong_adoption =
        GlobalLookupProofSessionEntropySealV1::test_only_v1([1; 32], [2; 32]).unwrap();
    wrong_adoption
        .bind_source_opening_context_v1([3; 32])
        .unwrap();
    let _pending = wrong_adoption.sample_source_blinding_v1(0).unwrap();
    assert!(
        wrong_adoption
            .adopt_source_commitment_v1(1, &generator)
            .is_err()
    );
    assert!(wrong_adoption.live.is_none());

    let mut unavailable = GlobalLookupProofSessionEntropySealV1::test_only_with_fault_v1(
        [1; 32],
        [2; 32],
        TestEntropyFaultV1::ErrorAt(0),
    )
    .unwrap();
    unavailable.bind_source_opening_context_v1([3; 32]).unwrap();
    assert!(unavailable.sample_source_blinding_v1(0).is_err());
    assert!(unavailable.live.is_none());

    let mut zero = GlobalLookupProofSessionEntropySealV1::test_only_with_fault_v1(
        [1; 32],
        [2; 32],
        TestEntropyFaultV1::ZeroAt(0),
    )
    .unwrap();
    zero.bind_source_opening_context_v1([3; 32]).unwrap();
    assert!(zero.sample_source_blinding_v1(0).is_err());
    assert!(zero.live.is_none());

    let unwind = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut session = GlobalLookupProofSessionEntropySealV1::test_only_with_fault_v1(
            [1; 32],
            [2; 32],
            TestEntropyFaultV1::PanicAt(0),
        )
        .unwrap();
        session.bind_source_opening_context_v1([3; 32]).unwrap();
        let _ = session.sample_source_blinding_v1(0);
    }));
    assert!(unwind.is_err());
}

#[test]
fn authority_chain_and_non_authority_surfaces_are_structurally_frozen() {
    assert!(SESSION_SOURCE_V1.lines().count() <= 750);
    assert!(SESSION_SOURCE_V1.len() <= 32_000);
    assert!(SESSION_TEST_SOURCE_V1.lines().count() <= 450);
    assert!(SESSION_TEST_SOURCE_V1.len() <= 20_000);
    for required in [
        "original_random: R",
        "commitment_entropy_bytes: u64",
        "GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1",
        "let mut live = self\n            .live\n            .take()",
        "pending_source: Option<GlobalLookupCommitmentCoordinateV1>",
        "slots.resize_with(exact_capacity, || None)",
        "INVENTORY_SKELETON_NEW_FILE_BYTES_V1: u64 = 0",
        "INVENTORY_SKELETON_NEW_IO_BYTES_V1: u64 = 0",
        "INVENTORY_SKELETON_NAMED_HEAP_BYTES_V1: usize",
        "COMPLETE_INVENTORY_MATERIALIZED_V1: bool = false",
    ] {
        assert!(
            SESSION_SOURCE_V1.contains(required),
            "missing guard: {required}"
        );
    }
    for required in [
        "proof_session: commitment_session_v1::RetainedSourceSessionV1<R>",
        ".complete_source_opening_v1(",
        ".adopt_source_commitment_v1(",
    ] {
        assert!(
            OPENING_SOURCE_V1.contains(required),
            "missing opening guard: {required}"
        );
    }
    assert!(REPLAY_SOURCE_V1.contains("replay.openings.validate_v1()?;"));
    assert!(REPLAY_SOURCE_V1.contains("openings: GlobalLookupSourceOpeningMaterialV1"));
    assert!(
        RADIX_SOURCE_V2
            .contains("evidence: Option<Phase23GlobalLookupSourceReplayEvidenceV1<R, K, P>>")
    );
    for forbidden in [
        "impl Clone for GlobalLookupCommitmentSessionV1",
        "impl Copy for GlobalLookupCommitmentSessionV1",
        "fn into_parts",
        "fn blindings_v1",
        "fn points_v1",
        "dyn Fn",
        "Serialize",
        "Deserialize",
        "Encode",
        "Decode",
    ] {
        assert!(
            !SESSION_SOURCE_V1.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
}
