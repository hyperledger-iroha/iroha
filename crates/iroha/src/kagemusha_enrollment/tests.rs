//! Actual middleware signatures, current-source invocation and fail-closed controls.

use super::*;
use iroha_data_model::kagemusha::{KagemushaEligibilityAuthorityV1, KagemushaEligibilityPurposeV1};
use std::cell::Cell;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn fixture() -> (KagemushaEligibilityPolicyV1, KagemushaEligibilityRequestV1) {
    let policy = KagemushaEligibilityPolicyV1 {
        version: 1,
        network_id: [1; 32],
        scheme_id: [2; 32],
        asset_digest: [3; 32],
        revision: 1,
        authority: KagemushaEligibilityAuthorityV1::Bank { fi_digest: [4; 32] },
        public_key: key(17).public_key().to_bytes().1.try_into().unwrap(),
        maximum_response_ms: 1000,
    };
    let request = KagemushaEligibilityRequestV1 {
        version: 1,
        policy_digest: policy.policy_digest().unwrap(),
        account_digest: [5; 32],
        actor_digest: [6; 32],
        attempt_id: [7; 32],
        nonce: [8; 32],
        operation_digest: [9; 32],
        purpose: KagemushaEligibilityPurposeV1::PreKeyPermit,
        requested_at_ms: 1000,
        expires_at_ms: 2000,
    };
    (policy, request)
}
fn current() -> CurrentEnrollmentEligibilityV1 {
    CurrentEnrollmentEligibilityV1 {
        approved: true,
        frozen: false,
        revision: 1,
        observed_at_ms: 1100,
    }
}

#[test]
fn bank_adapter_reads_current_scope_once_and_freeze_always_wins() {
    let (policy, request) = fixture();
    let original = request.encode_canonical(&policy).unwrap();
    for approved in [false, true] {
        for frozen in [false, true] {
            let reads = Cell::new(0);
            let mut times = [1000, 1100, 1200].into_iter();
            let bytes = answer_enrollment_eligibility_v1(
                &policy,
                &original,
                || Ok(times.next().unwrap()),
                |p, r| {
                    assert_eq!(p, &policy);
                    assert_eq!(r, &request);
                    reads.set(reads.get() + 1);
                    Ok(CurrentEnrollmentEligibilityV1 {
                        approved,
                        frozen,
                        revision: 12,
                        observed_at_ms: 1100,
                    })
                },
                |message| sign_enrollment_eligibility_v1(&key(17), message),
            )
            .unwrap();
            let response =
                KagemushaEligibilityResponseV1::decode_canonical(&bytes, &policy, &request, 1200)
                    .unwrap();
            assert_eq!(reads.get(), 1);
            assert_eq!(response.body.observed_at_ms, 1100);
            assert_eq!(response.body.source_revision, 12);
            assert_eq!(
                response.body.decision,
                if frozen {
                    KagemushaEligibilityDecisionV1::Frozen
                } else if approved {
                    KagemushaEligibilityDecisionV1::ApprovedUnfrozen
                } else {
                    KagemushaEligibilityDecisionV1::NotApproved
                }
            );
        }
    }
}

#[test]
fn invalid_and_foreign_requests_never_read_or_sign() {
    let (policy, request) = fixture();
    let mut foreign = request;
    foreign.policy_digest = [42; 32];
    for bytes in [
        vec![],
        vec![0; 2049],
        norito::encode_canonical(&foreign).unwrap(),
    ] {
        assert_eq!(
            answer_enrollment_eligibility_v1(
                &policy,
                &bytes,
                || panic!("invalid request must precede clock"),
                |_, _| panic!("invalid request must precede lookup"),
                |_| panic!("invalid request must precede signing")
            ),
            Err(EnrollmentEligibilityMiddlewareErrorV1::InvalidRequest)
        );
    }
}

#[test]
fn unavailable_current_state_and_invalid_revision_never_sign() {
    let (policy, request) = fixture();
    let original = request.encode_canonical(&policy).unwrap();
    for observation in [
        Err(EnrollmentEligibilityMiddlewareErrorV1::Unavailable),
        Ok(CurrentEnrollmentEligibilityV1 {
            revision: 0,
            ..current()
        }),
    ] {
        let expected = if observation.is_err() {
            EnrollmentEligibilityMiddlewareErrorV1::Unavailable
        } else {
            EnrollmentEligibilityMiddlewareErrorV1::InvalidObservation
        };
        assert_eq!(
            answer_enrollment_eligibility_v1(
                &policy,
                &original,
                || Ok(1100),
                |_, _| observation,
                |_| panic!("failed source must not sign")
            ),
            Err(expected)
        );
    }
}

#[test]
fn each_new_request_reads_updated_bank_state_and_cannot_reuse_previous_approval() {
    let (policy, mut request) = fixture();
    let mut previous: Option<Vec<u8>> = None;
    for frozen in [false, true] {
        request.nonce[0] += 1;
        let original = request.encode_canonical(&policy).unwrap();
        let bytes = answer_enrollment_eligibility_v1(
            &policy,
            &original,
            || Ok(1100),
            |_, _| {
                Ok(CurrentEnrollmentEligibilityV1 {
                    frozen,
                    ..current()
                })
            },
            |m| sign_enrollment_eligibility_v1(&key(17), m),
        )
        .unwrap();
        let response =
            KagemushaEligibilityResponseV1::decode_canonical(&bytes, &policy, &request, 1100)
                .unwrap();
        assert_eq!(
            response.body.decision,
            if frozen {
                KagemushaEligibilityDecisionV1::Frozen
            } else {
                KagemushaEligibilityDecisionV1::ApprovedUnfrozen
            }
        );
        if let Some(bytes) = previous {
            assert!(
                KagemushaEligibilityResponseV1::decode_canonical(&bytes, &policy, &request, 1100)
                    .is_err()
            );
        }
        previous = Some(bytes);
    }
}

#[test]
fn source_observation_time_cannot_be_relabelled_by_a_slow_lookup() {
    let (policy, request) = fixture();
    let original = request.encode_canonical(&policy).unwrap();
    for observed_at_ms in [1000, 1099, 1201] {
        let mut times = [1100, 1200].into_iter();
        assert_eq!(
            answer_enrollment_eligibility_v1(
                &policy,
                &original,
                || Ok(times.next().unwrap()),
                |_, _| Ok(CurrentEnrollmentEligibilityV1 {
                    observed_at_ms,
                    ..current()
                }),
                |_| panic!("old or future observation cannot be signed")
            ),
            Err(EnrollmentEligibilityMiddlewareErrorV1::InvalidObservation)
        );
    }
    let mut times = [1100, 1800, 1900].into_iter();
    let bytes = answer_enrollment_eligibility_v1(
        &policy,
        &original,
        || Ok(times.next().unwrap()),
        |_, _| Ok(current()),
        |m| sign_enrollment_eligibility_v1(&key(17), m),
    )
    .unwrap();
    let response =
        KagemushaEligibilityResponseV1::decode_canonical(&bytes, &policy, &request, 1900).unwrap();
    assert_eq!(response.body.observed_at_ms, 1100);
}

#[test]
fn expiry_or_clock_rollback_before_lookup_after_lookup_or_after_signing_refuses_output() {
    let (policy, request) = fixture();
    let original = request.encode_canonical(&policy).unwrap();
    for clock in [
        [999, 1100, 1200],
        [2000, 2000, 2000],
        [1100, 1099, 1200],
        [1100, 2000, 2000],
        [1100, 1200, 1199],
        [1100, 1200, 2000],
    ] {
        let mut times = clock.into_iter();
        let mut reads = 0;
        let mut signs = 0;
        assert_eq!(
            answer_enrollment_eligibility_v1(
                &policy,
                &original,
                || Ok(times.next().unwrap()),
                |_, _| {
                    reads += 1;
                    Ok(current())
                },
                |m| {
                    signs += 1;
                    sign_enrollment_eligibility_v1(&key(17), m)
                }
            ),
            Err(EnrollmentEligibilityMiddlewareErrorV1::Clock)
        );
        assert_eq!(reads, usize::from(clock[0] >= 1000 && clock[0] < 2000));
        assert_eq!(
            signs,
            usize::from(
                clock[0] >= 1000 && clock[0] < 2000 && clock[1] >= clock[0] && clock[1] < 2000
            )
        );
    }
}

#[test]
fn signer_failure_foreign_key_and_corrupt_signature_never_escape_as_success() {
    let (policy, request) = fixture();
    let original = request.encode_canonical(&policy).unwrap();
    let body_signature = answer_enrollment_eligibility_v1(
        &policy,
        &original,
        || Ok(1100),
        |_, _| Ok(current()),
        |_| Err(EnrollmentEligibilityMiddlewareErrorV1::Signing),
    );
    assert_eq!(
        body_signature,
        Err(EnrollmentEligibilityMiddlewareErrorV1::Signing)
    );
    for foreign in [false, true] {
        let result = answer_enrollment_eligibility_v1(
            &policy,
            &original,
            || Ok(1100),
            |_, _| Ok(current()),
            |m| {
                if foreign {
                    sign_enrollment_eligibility_v1(&key(18), m)
                } else {
                    Ok([0; 64])
                }
            },
        );
        assert_eq!(
            result,
            Err(EnrollmentEligibilityMiddlewareErrorV1::Signature)
        );
    }
    let other = KeyPair::from_seed(vec![18; 32], Algorithm::Secp256k1);
    assert_eq!(
        sign_enrollment_eligibility_v1(&other, &[1; 32]),
        Err(EnrollmentEligibilityMiddlewareErrorV1::Signing)
    );
}

#[test]
fn scheme_operator_requires_explicit_selection_and_never_implies_bank_kyc() {
    let (mut policy, mut request) = fixture();
    policy.authority = KagemushaEligibilityAuthorityV1::SchemeOperator {
        operator_digest: [33; 32],
    };
    request.policy_digest = policy.policy_digest().unwrap();
    let original = request.encode_canonical(&policy).unwrap();
    let bytes = answer_enrollment_eligibility_v1(
        &policy,
        &original,
        || Ok(1100),
        |p, _| {
            assert_eq!(p.authority, policy.authority);
            Ok(current())
        },
        |m| sign_enrollment_eligibility_v1(&key(17), m),
    )
    .unwrap();
    assert!(
        KagemushaEligibilityResponseV1::decode_canonical(&bytes, &policy, &request, 1100).is_ok()
    );
    policy.authority = KagemushaEligibilityAuthorityV1::Bank {
        fi_digest: [33; 32],
    };
    assert_eq!(
        answer_enrollment_eligibility_v1(
            &policy,
            &original,
            || Ok(1100),
            |_, _| panic!("changed authority must precede lookup"),
            |_| panic!("changed authority must precede signing")
        ),
        Err(EnrollmentEligibilityMiddlewareErrorV1::InvalidRequest)
    );
}
