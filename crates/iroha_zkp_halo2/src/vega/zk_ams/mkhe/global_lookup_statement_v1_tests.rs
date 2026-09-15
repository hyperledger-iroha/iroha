//! Canonical native40 persistent inventory and retired-role rejection controls.
use super::*;

#[test]
fn exact_roles_counts_phases_and_exclusive_storage_ranges_match_current_native_owners() {
    let expected = [
        (GlobalLookupCommitmentPurposeV1::Source, 0, 344),
        (
            GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow,
            344,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ExistingSumLow,
            6_192,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop,
            12_040,
            344,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorSumTop,
            12_384,
            344,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit,
            12_728,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorBorrow,
            18_576,
            6_192,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorMixedTop,
            24_768,
            344,
        ),
        (GlobalLookupCommitmentPurposeV1::SmallSigned, 25_112, 1_032),
        (
            GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude,
            26_144,
            1_032,
        ),
        (GlobalLookupCommitmentPurposeV1::QMaskDigit, 27_176, 6_400),
        (
            GlobalLookupCommitmentPurposeV1::QMaskComplementDigit,
            33_576,
            6_400,
        ),
        (GlobalLookupCommitmentPurposeV1::Multiplicity, 39_976, 1),
        (
            GlobalLookupCommitmentPurposeV1::InverseProductMask,
            39_977,
            1,
        ),
        (
            GlobalLookupCommitmentPurposeV1::SharedDifferenceInverse,
            39_978,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::SharedSumInverse,
            45_826,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::ComparatorDifferenceInverse,
            51_674,
            5_848,
        ),
        (
            GlobalLookupCommitmentPurposeV1::SmallPositiveInverse,
            57_522,
            1_032,
        ),
        (
            GlobalLookupCommitmentPurposeV1::SmallNegativeInverse,
            58_554,
            1_032,
        ),
        (
            GlobalLookupCommitmentPurposeV1::QMaskDigitInverse,
            59_586,
            6_400,
        ),
        (
            GlobalLookupCommitmentPurposeV1::QMaskComplementInverse,
            65_986,
            6_400,
        ),
    ];
    assert_eq!(ALL_POINT_PURPOSES_V1, expected.map(|(role, _, _)| role));
    let mut seen = vec![false; GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as usize];
    for (purpose, first, count) in expected {
        assert_eq!(purpose.count_v1(), count as usize);
        assert_eq!(first_commitment_ordinal_v1(purpose), first);
        for offset in 0..count {
            let coordinate = commitment_coordinate_v1(first + offset).unwrap();
            assert_eq!(coordinate.purpose, purpose);
            assert_eq!(coordinate.purpose_ordinal, offset);
            assert_eq!(
                coordinate.phase,
                if first < 39_978 {
                    GlobalLookupCommitmentPhaseV1::ChallengeIndependent
                } else {
                    GlobalLookupCommitmentPhaseV1::PostZ
                }
            );
            assert!(!seen[(first + offset) as usize]);
            seen[(first + offset) as usize] = true;
        }
    }
    assert!(seen.into_iter().all(|used| used));
    assert_eq!(GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1, 72_386);
    assert_eq!(PRE_Z_POINT_PURPOSES_V1.len(), 13);
    assert_eq!(
        PRE_Z_POINT_PURPOSES_V1
            .iter()
            .map(|r| r.count_v1())
            .sum::<usize>(),
        39_634
    );
    assert_eq!(
        POST_Z_POINT_PURPOSES_V1
            .iter()
            .map(|r| r.count_v1())
            .sum::<usize>(),
        32_408
    );
    for invalid in [72_386, 82_804, 82_805, u32::MAX] {
        assert!(commitment_coordinate_v1(invalid).is_err());
    }
}

#[test]
fn source_and_wire_orders_do_not_duplicate_shared_inverses_or_invent_residuals() {
    assert_eq!(GlobalLookupCommitmentPurposeV1::Source as u8, 1);
    assert_eq!(
        PRE_Z_POINT_PURPOSES_V1.map(|p| p as u8),
        [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert_eq!(
        POST_Z_POINT_PURPOSES_V1.map(|p| p as u8),
        [15, 16, 17, 18, 19, 20, 21]
    );
    assert_eq!(GlobalLookupCommitmentPurposeV1::Multiplicity.count_v1(), 1);
    assert_eq!(
        GlobalLookupCommitmentPurposeV1::InverseProductMask.count_v1(),
        1
    );
    assert_ne!(
        GlobalLookupCommitmentPurposeV1::Multiplicity,
        GlobalLookupCommitmentPurposeV1::InverseProductMask
    );
    assert_eq!(
        40 * 5 * 8 * 4,
        GlobalLookupCommitmentPurposeV1::QMaskDigit.count_v1()
    );
    assert_eq!(
        COMPARATOR_SIGNED_POINT_PURPOSES_V1
            .iter()
            .map(|p| p.count_v1())
            .sum::<usize>(),
        9288
    );
    for (logical, physical) in [
        (0, 12040),
        (343, 12383),
        (344, 12384),
        (687, 12727),
        (688, 18576),
        (6879, 24767),
        (6880, 24768),
        (7223, 25111),
        (7224, 25112),
        (8255, 26143),
        (8256, 26144),
        (9287, 27175),
    ] {
        assert_eq!(
            comparator_signed_coordinate_v1(logical)
                .unwrap()
                .global_ordinal,
            physical
        );
    }
    for retired in [9288, 9289, 9290, u32::MAX] {
        assert!(comparator_signed_coordinate_v1(retired).is_err());
    }
    let s3 = include_str!("rns_native_comparator_product.rs");
    let s5 = include_str!("rns_native_comparator_range_carry_product.rs");
    let s8 = include_str!("rns_native_small_sign_disjointness_product.rs");
    assert!(s3.contains("no-aggregate-residual"));
    assert!(s5.contains("no-aggregate-residual"));
    assert!(s8.contains("no-residual-q8"));
}

#[test]
fn current_inventory_identity_matches_independent_preimage_and_rejects_retired_identity() {
    assert_eq!(
        hex::encode(global_lookup_topology_digest_v1()),
        "e431e50523174f941a0404df0747f18e7f86fe8456badb8f8a7a709e373c8a3f"
    );
    assert_ne!(
        hex::encode(global_lookup_topology_digest_v1()),
        "3af9a6ad67383c32b06bb5d95a05863b8cb0b3338660177bc2a92e1bbf40b4ab"
    );
    let original = global_lookup_topology_digest_v1();
    let mut frames = Vec::new();
    for role in ALL_POINT_PURPOSES_V1 {
        frames.push([role.phase_v1() as u8, role as u8]);
    }
    let reference = |frames: &[[u8; 2]]| {
        let mut hash = Keccak256::new();
        hash.update(TOPOLOGY_DOMAIN_V1);
        hash.update(&[1]);
        for count in [40_u32, 344, 17, 18, 1032, 6400, 72386] {
            hash.update(&count.to_be_bytes());
        }
        let mut first = 0_u32;
        for (frame, role) in frames.iter().zip(ALL_POINT_PURPOSES_V1) {
            hash.update(frame);
            hash.update(&first.to_be_bytes());
            hash.update(&(role.count_v1() as u32).to_be_bytes());
            first += role.count_v1() as u32;
        }
        hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
        hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
        hash.update(&(INVENTORY_LANGUAGE_V1.len() as u16).to_be_bytes());
        hash.update(INVENTORY_LANGUAGE_V1);
        hash.finalize()
    };
    assert_eq!(reference(&frames), original);
    for index in 0..frames.len() {
        let mut altered = frames.clone();
        altered[index][0] ^= 3;
        assert_ne!(reference(&altered), original);
    }
    let mut reordered = frames.clone();
    reordered.swap(1, 2);
    assert_ne!(reference(&reordered), original);
}

#[test]
fn inventory_is_metadata_and_registers_no_second_transcript_or_source_authority() {
    let source = include_str!("global_lookup_statement_v1.rs");
    let verifier = include_str!("rns_native_global_lookup_z_commitment_view.rs");
    assert_eq!(
        source
            .matches("mod vector_arithmetic_plane_openings_v1;")
            .count(),
        1
    );
    for retired in [
        "mod challenge_v1;",
        "mod challenge_v2;",
        "GlobalLookupChallengeState",
        "struct GlobalLookupProofSession",
        "Production {",
        "TestOnly(",
        "ReleaseAuthorization",
        "Scalar::from",
    ] {
        assert!(!source.contains(retired), "{retired}");
    }
    assert!(verifier.contains("global_lookup_statement_v1::{"));
    assert!(!verifier.contains("enum PhysicalPurposeV1"));
    assert!(!verifier.contains("impl GlobalLookupCommitmentPurposeV1"));
}
