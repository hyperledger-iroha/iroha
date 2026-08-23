use super::*;

use crate::zk::kagemusha_sha256_table16_v4::Table16Chip;
use halo2_proofs::{halo2curves::pasta::Fp, plonk::ConstraintSystem, poly::Rotation};

#[test]
fn fixed_inventory_pins_all_nine_existing_helper_jobs() {
    let expected = [
        (OfflineCashSha256JobV1::CurrentGuardBinding, 355, 6),
        (OfflineCashSha256JobV1::NextGuardBinding, 432, 7),
        (OfflineCashSha256JobV1::PlatformMessage, 494, 8),
        (OfflineCashSha256JobV1::PlatformPublicKeySec1, 65, 2),
        (OfflineCashSha256JobV1::GuardUseClaim, 533, 9),
        (OfflineCashSha256JobV1::PlatformBindClaim, 376, 7),
        (OfflineCashSha256JobV1::AndroidIssuerPublicKeySec1, 65, 2),
        (OfflineCashSha256JobV1::AndroidKeyCertClaim, 480, 8),
        (OfflineCashSha256JobV1::GuardBundle, 619, 10),
    ];
    assert_eq!(OFFLINE_CASH_FIXED_SHA256_JOBS_V1.len(), expected.len());
    for (actual, (job, message_bytes, blocks)) in
        OFFLINE_CASH_FIXED_SHA256_JOBS_V1.iter().zip(expected)
    {
        assert_eq!(actual.job, job);
        assert_eq!(actual.exact_message_bytes, message_bytes);
        assert_eq!(actual.compression_blocks, blocks);
        assert_eq!(sha256_compression_blocks(message_bytes), Ok(blocks));
    }
    assert_eq!(
        checked_block_sum(&fixed_job_block_counts()),
        Ok(59),
        "the nine existing helper jobs occupy exactly 59 SHA-256 blocks"
    );
}

#[test]
fn canonical_sha_padding_boundaries_are_exact() {
    assert_eq!(sha256_compression_blocks(0), Ok(1));
    assert_eq!(sha256_compression_blocks(55), Ok(1));
    assert_eq!(sha256_compression_blocks(56), Ok(2));
    assert_eq!(sha256_compression_blocks(63), Ok(2));
    assert_eq!(sha256_compression_blocks(64), Ok(2));
    assert_eq!(sha256_compression_blocks(119), Ok(2));
    assert_eq!(sha256_compression_blocks(120), Ok(3));
}

#[test]
fn raw_tbs_job_requires_an_external_cap_and_rejects_cap_plus_one() {
    assert_eq!(
        OfflineCashRawTbsSha256BoundV1::new(56, None),
        Err(OfflineCashCompactSha256ErrorV1::MissingGovernedRawTbsCap)
    );
    assert_eq!(
        OfflineCashRawTbsSha256BoundV1::new(0, Some(1)),
        Err(OfflineCashCompactSha256ErrorV1::EmptyRawTbsCertificate)
    );
    assert_eq!(
        OfflineCashRawTbsSha256BoundV1::new(1, Some(0)),
        Err(OfflineCashCompactSha256ErrorV1::EmptyGovernedRawTbsCap)
    );
    assert_eq!(
        OfflineCashRawTbsSha256BoundV1::new(57, Some(56)),
        Err(
            OfflineCashCompactSha256ErrorV1::RawTbsCertificateExceedsCap {
                exact_message_bytes: 57,
                governed_max_message_bytes: 56,
            }
        )
    );

    // These padding-boundary values exercise the API only; neither is a
    // production TBS policy cap.
    let bounded =
        OfflineCashRawTbsSha256BoundV1::new(55, Some(56)).expect("synthetic test cap is explicit");
    assert_eq!(bounded.exact_compression_blocks, 1);
    assert_eq!(bounded.maximum_compression_blocks, 2);
    let inventory = OfflineCashSha256InventoryV1::new(bounded);
    assert_eq!(inventory.exact_block_counts()[9], 1);
    assert_eq!(inventory.maximum_block_counts()[9], 2);
    assert_eq!(inventory.exact_total_blocks(), Ok(60));
    assert_eq!(inventory.maximum_total_blocks(), Ok(61));
}

#[test]
fn nine_fixed_jobs_need_three_table16_lanes_at_k16() {
    let evidence = OfflineCashCompactSha256EvidenceV1::fixed_jobs().expect("shape arithmetic");
    assert_eq!(evidence.fixed_jobs, 9);
    assert_eq!(evidence.fixed_compression_blocks, 59);
    assert_eq!(evidence.minimum_table16_lanes, 3);
    assert_eq!(evidence.two_lane_max_rows, 68_026);
    assert_eq!(evidence.three_lane_max_rows, 45_372);
    assert!(evidence.two_lane_max_rows > K16_USABLE_ROWS_V1);
    assert!(evidence.three_lane_max_rows <= K16_USABLE_ROWS_V1);
}

#[test]
fn reviewed_table16_constraint_system_matches_pinned_lane_footprint() {
    let mut meta = ConstraintSystem::<Fp>::default();
    let _ = Table16Chip::<Fp>::configure_lanes::<3>(&mut meta);
    assert_eq!(meta.degree(), 9);
    assert_eq!(meta.num_advice_columns(), 33);
    assert_eq!(meta.advice_queries().len(), 90);
    assert_eq!(meta.num_fixed_columns(), 4);
    assert_eq!(meta.fixed_queries().len(), 4);
    assert_eq!(meta.num_selectors(), 66);
    assert_eq!(meta.lookups().len(), 12);
    assert_eq!(meta.permutation().get_columns().len(), 25);
    assert!(
        meta.advice_queries()
            .iter()
            .any(|(_, rotation)| *rotation != Rotation::cur())
    );
}

#[test]
fn exact_shape_and_optimistic_current_query_projection_both_fail_closed() {
    let evidence = OfflineCashCompactSha256EvidenceV1::fixed_jobs().expect("shape arithmetic");
    let reviewed = evidence.reviewed_shape;
    assert_eq!(reviewed.k, 16);
    assert_eq!(
        reviewed.query_model,
        OfflineCashSha256QueryModelV1::ReviewedTable16
    );
    assert_eq!(reviewed.degree, 9);
    assert_eq!(reviewed.lanes, 3);
    assert_eq!(reviewed.advice_columns, 33);
    assert_eq!(reviewed.advice_queries, 90);
    assert_eq!(reviewed.instance_columns, 1);
    assert_eq!(reviewed.instance_queries, 1);
    assert_eq!(reviewed.fixed_columns, 5);
    assert_eq!(reviewed.fixed_queries, 5);
    assert_eq!(reviewed.selectors, 66);
    assert_eq!(reviewed.lookup_arguments, 12);
    assert_eq!(reviewed.equality_columns, 25);
    assert_eq!(reviewed.permutation_chunks, 4);
    assert_eq!(reviewed.quotient_pieces, 9);
    assert_eq!(reviewed.opening_point_sets, 5);
    assert_eq!(reviewed.proof_points, 116);
    assert_eq!(reviewed.proof_scalars, 265);
    assert_eq!(reviewed.raw_proof_bytes, 12_192);
    assert_eq!(reviewed.augmented_proof_bytes, 12_224);

    let projected = evidence.optimistic_current_only_lower_bound;
    assert_eq!(
        projected.query_model,
        OfflineCashSha256QueryModelV1::OptimisticCurrentOnlyLowerBound
    );
    assert_eq!(projected.opening_point_sets, 4);
    assert_eq!(projected.proof_points, 116);
    assert_eq!(projected.proof_scalars, 264);
    assert_eq!(projected.raw_proof_bytes, 12_160);
    assert_eq!(projected.augmented_proof_bytes, 12_192);
    assert!(projected.advice_columns > OFFLINE_CASH_SHA256_MAX_ADVICE_V1);
    assert!(projected.augmented_proof_bytes > OFFLINE_CASH_SHA256_MAX_AUGMENTED_PROOF_BYTES_V1);

    let one_lane = OfflineCashSha256ChildShapeV1::for_lanes(
        1,
        OfflineCashSha256QueryModelV1::OptimisticCurrentOnlyLowerBound,
    )
    .expect("one-lane lower bound");
    assert_eq!(one_lane.augmented_proof_bytes, 5_344);
    assert!(one_lane.augmented_proof_bytes > OFFLINE_CASH_SHA256_MAX_AUGMENTED_PROOF_BYTES_V1);

    assert_eq!(
        evidence.blockers_without_governed_tbs_cap(),
        [
            OfflineCashCompactSha256BlockerV1::MissingGovernedRawTbsCap,
            OfflineCashCompactSha256BlockerV1::NoReviewedCurrentQueryCircuit,
            OfflineCashCompactSha256BlockerV1::AdviceEnvelopeExceeded {
                actual: 33,
                maximum: 8,
            },
            OfflineCashCompactSha256BlockerV1::AugmentedProofEnvelopeExceeded {
                actual: 12_192,
                maximum: 3_200,
            },
        ]
    );
}

#[test]
fn source_guard_keeps_inventory_and_authority_boundary_explicit() {
    let helper = include_str!("helper_relation.rs");
    for domain in [
        "iroha:offline-cash:v1:helper:current-guard",
        "iroha:offline-cash:v1:helper:next-guard",
        "iroha:offline-cash:v1:helper:platform-message",
        "iroha:offline-cash:v1:helper:guard-use-claim",
        "iroha:offline-cash:v1:helper:platform-bind-claim",
        "iroha:offline-cash:v1:helper:android-key-cert-claim",
        "iroha:offline-cash:v1:helper:guard-bundle",
    ] {
        assert!(helper.contains(domain));
    }
    assert!(helper.contains("Sha256::digest(platform_public_key.as_sec1_bytes())"));
    assert!(helper.contains("Sha256::digest(android.issuer_public_key_sec1)"));

    let source = include_str!("sha256_compact.rs");
    assert!(source.contains("evidence, not a circuit"));
    assert!(source.contains("MissingGovernedRawTbsCap"));
    assert!(!source.contains("impl Circuit<"));
    assert!(!source.contains("verify_proof"));
    assert!(!source.contains("VerificationAvailable"));
}
