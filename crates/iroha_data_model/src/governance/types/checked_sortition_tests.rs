//! Checked sortition identities retain installed bytes, verdicts and original refusals.

use super::*;

#[test]
fn borrowed_governance_elements_keep_the_owned_sequence_payload() {
    for values in [Vec::<u64>::new(), vec![0, 127, 256, u64::MAX]] {
        for flags in [0, norito::core::default_encode_flags()] {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            let mut expected = Vec::new();
            norito::core::serialize_to_buffer(&values, &mut expected).unwrap();
            let mut actual = Vec::new();
            norito::core::serialize_to_buffer(&GovernanceElements(&values), &mut actual).unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn borrowed_governance_elements_retain_original_leaf_allocation_refusal() {
    #[expect(
        clippy::disallowed_types,
        reason = "Norito allocates canonical sorting scratch for unordered maps"
    )]
    let values = [std::collections::HashMap::from([(3_u64, 7_u64), (1, 9)])];
    let original = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || {
            crate::governance_fingerprint::checked_fingerprint(
                crate::governance_fingerprint::CANDIDATE_ROOT_V1,
                &GovernanceElements(&values),
            )
        },
    )
    .unwrap_err()
    .into_local_refusal()
    .unwrap();
    assert_eq!(
        original.kind(),
        norito::core::DecodeLocalRefusalKind::EnclosingLimit
    );
}

fn request() -> SortitionRequestV1 {
    let governance = GovernanceAttemptId::new([1; 32]);
    let election = BodyElectionAttemptId::derive_v1(governance, ParliamentBody::PolicyJury, 0);
    SortitionRequestV1::try_new_canonical(
        governance,
        election,
        ParliamentBody::PolicyJury,
        [2; 32],
        40,
        30,
        10,
        12,
        BeaconSessionId::new([3; 32]),
        None,
    )
    .unwrap()
}

#[test]
fn checked_sortition_identities_preserve_original_preimages_without_output_allocation() {
    let request = request();
    let expected_request = request.canonical_id();
    let expected_election =
        BodyElectionAttemptId::derive_v1(request.governance_attempt_id, request.body, u32::MAX);
    norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            assert_eq!(request.canonical_id_checked_v1().unwrap(), expected_request);
            assert_eq!(
                BodyElectionAttemptId::derive_checked_v1(
                    request.governance_attempt_id,
                    request.body,
                    u32::MAX,
                )
                .unwrap(),
                expected_election
            );
            request.validate_checked_v1(None).unwrap();
        },
    );
}

#[test]
fn checked_sortition_validation_preserves_ordered_invariant_verdicts() {
    let original = request();
    let mut cases = vec![original];
    let mutations: [fn(&mut SortitionRequestV1); 8] = [
        |value| value.candidate_count = 0,
        |value| value.candidate_count = MAX_PARLIAMENT_CITIZENS_V1 + 1,
        |value| value.target_seats = 0,
        |value| value.target_seats = MAX_PARLIAMENT_BODY_TARGET_SEATS_V1 + 1,
        |value| value.request_height = 0,
        |value| value.pulse_height = 0,
        |value| value.pulse_height = value.request_height,
        |value| value.candidate_root = [0; 32],
    ];
    for mutate in mutations {
        let mut candidate = original;
        mutate(&mut candidate);
        candidate.id = candidate.canonical_id();
        cases.push(candidate);
    }
    let mut noncanonical = original;
    noncanonical.id = SortitionRequestId::new([9; 32]);
    cases.push(noncanonical);
    let mut zero = original;
    zero.id = SortitionRequestId::new([0; 32]);
    cases.push(zero);
    for candidate in cases {
        for consumed in [None, Some(11), Some(12), Some(13)] {
            let checked = candidate.validate_checked_v1(consumed).map_err(|error| {
                let CheckedSortitionRequestErrorV1::Rejected(error) = error else {
                    panic!("ordinary request has no local codec refusal");
                };
                error
            });
            assert_eq!(checked, candidate.validate(consumed));
        }
    }
}

#[test]
fn checked_sortition_validation_keeps_original_identity_refusal_and_precedence() {
    let original = request();
    let refusal = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || norito::core::classify_decode_attempt(|| norito::core::reserve_decode_allocation(1)),
    )
    .unwrap_err();
    let expected = refusal.local_refusal().unwrap();
    let error = original
        .validate_with_identity(None, false, || {
            Err(CheckedSortitionRequestErrorV1::Codec(refusal))
        })
        .unwrap_err();
    assert!(
        matches!(error, CheckedSortitionRequestErrorV1::Codec(error) if error.local_refusal() == Some(expected))
    );

    let mut zero = original;
    zero.id = SortitionRequestId::new([0; 32]);
    let error = zero
        .validate_with_identity::<CheckedSortitionRequestErrorV1>(None, false, || {
            panic!("zero bindings reject before identity encoding")
        })
        .unwrap_err();
    assert!(matches!(
        error,
        CheckedSortitionRequestErrorV1::Rejected(SortitionRequestErrorV1::ZeroBinding)
    ));
}

#[test]
fn checked_borrowed_election_preimages_preserve_owned_candidate_and_draw_roots() {
    let request = request();
    let candidates: Vec<_> = [4, 5]
        .map(|seed| {
            AccountId::new(
                iroha_crypto::KeyPair::from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            )
        })
        .into();
    let assignments: Vec<_> = candidates
        .iter()
        .map(|member| {
            let expected = AssignmentId::derive_v1(request.body_election_attempt_id, member);
            assert_eq!(
                AssignmentId::derive_checked_v1(request.body_election_attempt_id, member).unwrap(),
                expected
            );
            ParliamentSeatAssignmentV1 {
                assignment_id: expected,
                member: member.clone(),
            }
        })
        .collect();
    for count in 0..=candidates.len() {
        assert_eq!(
            checked_parliament_candidate_root_v1(
                request.governance_attempt_id,
                request.body,
                &candidates[..count],
            )
            .unwrap(),
            parliament_candidate_root_v1(
                request.governance_attempt_id,
                request.body,
                &candidates[..count],
            )
        );
        assert_eq!(
            checked_parliament_assignment_plan_root_v1(
                request.body_election_attempt_id,
                &assignments[..count],
                &assignments[count..],
                2,
            )
            .unwrap(),
            parliament_assignment_plan_root_v1(
                request.body_election_attempt_id,
                &assignments[..count],
                &assignments[count..],
                2,
            )
        );
    }
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([8; 32]),
    ));
    assert_eq!(
        BeaconSessionId::for_network_checked_v1(&network).unwrap(),
        BeaconSessionId::for_network_v1(&network)
    );
}
