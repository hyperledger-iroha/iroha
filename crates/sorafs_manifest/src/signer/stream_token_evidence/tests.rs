//! Independently signed observer simulations; these tests do not qualify deployed hardware.

use super::*;
use crate::signer::{custody::*, receipt::*, state_observation::*, stream_token::*};
use crate::token::StreamTokenV1;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use norito::codec::Encode;

use crate::signer::stream_token::receipt_test_support as receipt_fixture;
#[path = "observer_fixture.rs"]
mod test_support;
#[path = "wire_tests.rs"]
mod wire_tests;
use test_support::*;

type EvidenceError = SignerStreamTokenEvidenceErrorV1;
type ReceiptError = SignerStreamTokenReceiptErrorV1;
type Phase = SignerStreamTokenObservationPhaseV1;

#[test]
fn every_phase_produces_only_its_current_completed_or_release_marker() {
    for phase in phases() {
        let mut evidence = Evidence::new(phase);
        assert_eq!(evidence.request.magic, *b"IRSTKQ01");
        assert_eq!(evidence.state.body.magic, *b"IRSTKS01");
        let pending_debug = format!("{:?}", evidence.attempt.as_ref().expect("pending request"));
        for value in [
            &evidence.receipt.binding.key_handle,
            &evidence.receipt.token.body.token_id,
        ] {
            assert!(!pending_debug.contains(value));
        }
        assert_positive(&mut evidence);
    }
}

#[test]
fn independent_request_and_observer_preimages_are_fixed_across_all_ten_layouts() {
    for phase in phases() {
        let mut baseline = Evidence::new(phase);
        let request = baseline
            .request
            .encode_canonical()
            .expect("canonical request");
        let state = baseline
            .state
            .encode_canonical()
            .expect("canonical observation");
        let payload = observer_payload(&baseline.state.body);
        assert_positive(&mut baseline);
        for flags in receipt_fixture::layouts() {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            let mut evidence = Evidence::new(phase);
            assert_eq!(
                evidence
                    .request
                    .encode_canonical()
                    .expect("request layout pin"),
                request
            );
            assert_eq!(
                evidence
                    .attempt
                    .as_ref()
                    .expect("retained attempt")
                    .request_bytes()
                    .expect("retained request bytes"),
                request
            );
            assert_eq!(
                evidence.request.digest().expect("request digest"),
                request_digest(&evidence.request)
            );
            assert_eq!(
                evidence.state.body.request_digest,
                request_digest(&evidence.request)
            );
            assert_eq!(
                evidence
                    .state
                    .body
                    .signing_payload()
                    .expect("state preimage"),
                payload
            );
            assert_eq!(
                evidence.state.encode_canonical().expect("state layout pin"),
                state
            );
            assert_eq!(
                SignerStreamTokenObservationRequestV1::decode_canonical(&request)
                    .expect("request roundtrip"),
                evidence.request
            );
            assert_eq!(
                SignerStreamTokenStateObservationV1::decode_canonical(&state)
                    .expect("state roundtrip"),
                evidence.state
            );
            match &evidence.request.subject {
                SignerStreamTokenObservationRequestSubjectV1::CurrentCustody { binding_digest } => {
                    assert!(is_current(phase));
                    assert_eq!(*binding_digest, evidence.receipt.expected.binding_digest());
                }
                SignerStreamTokenObservationRequestSubjectV1::CompletedOperation {
                    binding_digest,
                    operation_id,
                    signing_payload_digest,
                    signing_payload_size,
                    receipt_digest: frozen_receipt,
                    signatures_digest,
                } => {
                    assert!(!is_current(phase));
                    assert_eq!(*binding_digest, evidence.receipt.expected.binding_digest());
                    assert_eq!(*operation_id, evidence.receipt.expected.operation_id());
                    assert_eq!(
                        *signing_payload_digest,
                        evidence.receipt.expected.signing_payload_digest()
                    );
                    assert_eq!(
                        *signing_payload_size,
                        evidence.receipt.expected.signing_payload_size()
                    );
                    assert_eq!(*frozen_receipt, receipt_digest(&evidence.receipt_bytes));
                    assert_eq!(
                        *signatures_digest,
                        receipt_fixture::oracle_signatures(&evidence.receipt.receipt.signatures)
                    );
                }
            }
            assert_positive(&mut evidence);
        }
        let mut changed_phase = baseline.request.clone();
        changed_phase.phase = if phase == Phase::Startup {
            Phase::BeforeProvider
        } else {
            Phase::Startup
        };
        assert_ne!(
            request_digest(&changed_phase),
            request_digest(&baseline.request)
        );
    }
}

#[test]
fn owned_attempt_is_consumed_on_success_and_failure_and_recovery_uses_a_fresh_query() {
    let mut evidence = Evidence::new(Phase::BeforeRelease);
    let original_receipt = evidence.receipt_bytes.clone();
    let cached = evidence.observation_bytes();
    assert_positive(&mut evidence);
    assert!(
        evidence.attempt.take().is_none(),
        "bounded runtime cannot dispatch this attempt again"
    );
    let old_request = request_digest(&evidence.request);
    evidence.now += 1;
    evidence.state.body.observed_at_unix_ms += 1;
    evidence.state.body.expires_at_unix_ms += 1;
    evidence.fresh_attempt([0x92; 32], evidence.now);
    assert_ne!(request_digest(&evidence.request), old_request);
    assert_eq!(evidence.receipt_bytes, original_receipt);
    assert_eq!(
        evidence.verify_bytes(&cached).err(),
        Some(EvidenceError::SourceMismatch)
    );
    assert!(
        evidence.attempt.take().is_none(),
        "failed attempt cannot be reused either"
    );
    evidence.now += 1;
    evidence.state.body.observed_at_unix_ms += 1;
    evidence.state.body.expires_at_unix_ms += 1;
    evidence.fresh_attempt([0x93; 32], evidence.now);
    assert_positive(&mut evidence);
    assert_eq!(evidence.receipt_bytes, original_receipt);
    // Ownership above is a bounded runtime simulation. The pure verifier has no global cache and
    // does not promise to reject a separately reconstructed expected value with an old challenge.
}

#[test]
fn independently_pinned_observer_keys_and_every_identity_slot_are_separate() {
    for observer_slot in 0..2 {
        for other_slot in 0..5 {
            let mut evidence = checked_fixture(Phase::BeforeRelease);
            let reused = match other_slot {
                0 => evidence.receipt.binding.service_id.clone(),
                1 => evidence.receipt.binding.administrator_id.clone(),
                2 => evidence.receipt.trust.authority.service_id.clone(),
                3 => evidence.receipt.trust.authority.administrator_id.clone(),
                _ if observer_slot == 0 => evidence.trust.authority.administrator_id.clone(),
                _ => evidence.trust.authority.service_id.clone(),
            };
            if observer_slot == 0 {
                evidence.trust.authority.service_id = reused;
            } else {
                evidence.trust.authority.administrator_id = reused;
            }
            evidence.state.body.authority = evidence.trust.authority.clone();
            evidence.resign();
            assert_error(&mut evidence, EvidenceError::InvalidTrust);
        }
    }
    for key_slot in 0..3 {
        let mut evidence = checked_fixture(Phase::BeforeRelease);
        evidence.trust.public_key = match key_slot {
            0 => evidence.receipt.signer.public_key().clone(),
            1 => evidence.receipt.attester.public_key().clone(),
            _ => KeyPair::try_from_seed(vec![0x79; 32], Algorithm::Secp256k1)
                .expect("wrong observer algorithm")
                .public_key()
                .clone(),
        };
        assert_error(&mut evidence, EvidenceError::InvalidTrust);
    }
    let invalid_trust: &[fn(&mut Evidence)] = &[
        |e| e.trust.authority.service_id.clear(),
        |e| e.trust.authority.administrator_id.clear(),
        |e| e.trust.authority.key_revision = 0,
        |e| e.trust.authority.policy_revision = 0,
        |e| e.trust.authority.policy_digest = [0; 32],
        |e| e.trust.max_state_age_ms = 0,
        |e| e.trust.max_state_age_ms = 300_001,
        |e| e.trust.active_from_unix_ms = e.now + 1,
        |e| e.trust.active_until_unix_ms = e.now,
    ];
    for mutate in invalid_trust {
        let mut evidence = checked_fixture(Phase::Startup);
        mutate(&mut evidence);
        assert_error(&mut evidence, EvidenceError::InvalidTrust);
    }
}

#[test]
fn signed_responses_cannot_replace_caller_challenge_lower_bound_or_frozen_receipt_commitments() {
    let request_mutations: &[fn(&mut SignerStreamTokenObservationRequestV1)] = &[
        |r| r.challenge[0] ^= 1,
        |r| r.minimum_anchor.height += 1,
        |r| r.minimum_anchor.block_hash[0] ^= 1,
        |r| r.minimum_anchor.state_digest[0] ^= 1,
        |r| r.not_before_unix_ms += 1,
    ];
    for phase in [Phase::Startup, Phase::BeforeRelease] {
        for mutate in request_mutations {
            let mut evidence = checked_fixture(phase);
            let mut candidate_request = evidence.request.clone();
            mutate(&mut candidate_request);
            evidence.state.body.request_digest = request_digest(&candidate_request);
            evidence.resign();
            assert_error(&mut evidence, EvidenceError::SourceMismatch);
        }
    }
    for field in 0..6 {
        let mut evidence = checked_fixture(Phase::BeforeRelease);
        let mut candidate_request = evidence.request.clone();
        let SignerStreamTokenObservationRequestSubjectV1::CompletedOperation {
            binding_digest,
            operation_id,
            signing_payload_digest,
            signing_payload_size,
            receipt_digest,
            signatures_digest,
        } = &mut candidate_request.subject
        else {
            panic!("completed request")
        };
        match field {
            0 => binding_digest[0] ^= 1,
            1 => operation_id[0] ^= 1,
            2 => signing_payload_digest[0] ^= 1,
            3 => *signing_payload_size += 1,
            4 => receipt_digest[0] ^= 1,
            _ => signatures_digest[0] ^= 1,
        }
        evidence.state.body.request_digest = request_digest(&candidate_request);
        evidence.resign();
        assert_error(&mut evidence, EvidenceError::SourceMismatch);
    }
    let mut evidence = checked_fixture(Phase::BeforeRelease);
    let mut alternative = receipt_fixture::fixture();
    alternative.receipt.intent.previous_audit.sequence += 1;
    alternative.receipt.intent.previous_audit.digest[0] ^= 1;
    receipt_fixture::rebuild_operation(&mut alternative);
    receipt_fixture::assert_positive(&alternative);
    evidence.receipt_bytes = alternative
        .receipt
        .encode_canonical()
        .expect("different valid exact receipt");
    assert_error(&mut evidence, EvidenceError::SourceMismatch);
}

#[test]
fn observer_signature_authority_network_and_domain_must_match_independent_configuration() {
    let mutations: &[fn(&mut SignerStreamTokenStateObservationBodyV1)] = &[
        |s| s.authority.key_revision += 1,
        |s| s.authority.policy_revision += 1,
        |s| s.authority.policy_digest[0] ^= 1,
        |s| s.authority.service_id = "another-observer".into(),
        |s| s.authority.administrator_id = "another-security-team".into(),
        |s| s.chain_id = "another-chain".into(),
        |s| s.network_id[0] ^= 1,
    ];
    for mutate in mutations {
        let mut evidence = checked_fixture(Phase::BeforeRelease);
        mutate(&mut evidence.state.body);
        evidence.resign();
        assert_error(&mut evidence, EvidenceError::InvalidState);
    }
    let mut wrong_key = checked_fixture(Phase::Startup);
    wrong_key.observer = receipt_fixture::key(0x79);
    wrong_key.resign();
    assert_error(&mut wrong_key, EvidenceError::InvalidState);
    let mut malformed = checked_fixture(Phase::Startup);
    malformed.state.signature[0] ^= 1;
    assert_error(&mut malformed, EvidenceError::InvalidState);
    let mut wrong_domain = checked_fixture(Phase::BeforeRelease);
    let mut preimage = b"iroha.sorafs.release-manifest.finalized-state.v1\0".to_vec();
    preimage.extend(
        norito::encode_canonical(&wrong_domain.state.body).expect("same body under release domain"),
    );
    wrong_domain.state.signature =
        Signature::try_new(wrong_domain.observer.private_key(), &preimage)
            .expect("real wrong-domain signature")
            .payload()
            .try_into()
            .expect("Ed25519 width");
    assert_error(&mut wrong_domain, EvidenceError::InvalidState);
}

#[test]
fn current_finality_trusted_time_and_observation_floor_are_not_candidate_selected() {
    let mutations: &[fn(&mut Evidence)] = &[
        |e| e.state.body.observed_at_unix_ms = 0,
        |e| e.state.body.observed_at_unix_ms = e.now + 1,
        |e| e.state.body.observed_at_unix_ms = e.request.not_before_unix_ms - 1,
        |e| e.state.body.expires_at_unix_ms = e.now,
        |e| e.state.body.expires_at_unix_ms = e.state.body.observed_at_unix_ms,
        |e| e.state.body.expires_at_unix_ms = e.state.body.observed_at_unix_ms + 10_001,
        |e| e.state.body.current_anchor.height -= 1,
        |e| e.state.body.current_anchor.block_hash[0] ^= 1,
        |e| e.state.body.current_anchor.state_digest[0] ^= 1,
        |e| e.state.body.signer_revoked = true,
        |e| e.state.body.attester_revoked = true,
        |e| e.now = e.request.not_before_unix_ms - 1,
    ];
    for phase in [Phase::Startup, Phase::AfterCommit, Phase::BeforeRelease] {
        for mutate in mutations {
            let mut evidence = checked_fixture(phase);
            mutate(&mut evidence);
            evidence.resign();
            assert_error(&mut evidence, EvidenceError::InvalidState);
        }
    }
    let mut lower_time = checked_fixture(Phase::BeforeRelease);
    lower_time.state.body.observed_at_unix_ms = lower_time.request.not_before_unix_ms;
    lower_time.resign();
    assert_positive(&mut lower_time);
    let mut advancing = checked_fixture(Phase::BeforeRelease);
    advancing.state.body.current_anchor.height += 1;
    advancing.state.body.current_anchor.block_hash = [0x94; 32];
    advancing.resign();
    assert_positive(&mut advancing);
    let mut expiry_bound = checked_fixture(Phase::Startup);
    expiry_bound.trust.active_until_unix_ms = expiry_bound.state.body.expires_at_unix_ms - 1;
    assert_error(&mut expiry_bound, EvidenceError::InvalidState);
}

#[test]
fn signed_active_heads_and_completed_rows_still_require_original_custody_and_completion() {
    let active_mutations: &[fn(&mut SignerCustodyActiveHeadV1)] = &[
        |h| h.record_digest[0] ^= 1,
        |h| h.sequence += 1,
        |h| h.key_revision += 1,
        |h| h.policy_revision += 1,
        |h| h.policy_digest[0] ^= 1,
    ];
    for phase in [Phase::Startup, Phase::BeforeRelease] {
        for mutate in active_mutations {
            let mut evidence = checked_fixture(phase);
            mutate(&mut evidence.state.body.active_head);
            evidence.resign();
            assert_error(
                &mut evidence,
                EvidenceError::Receipt(ReceiptError::Custody(
                    SignerCustodyErrorV1::ReplayOrRollback,
                )),
            );
        }
    }
    let completed_mutations: &[fn(&mut SignerCompletedOperationV1)] = &[
        |c| c.operation_id[0] ^= 1,
        |c| c.intent_digest[0] ^= 1,
        |c| c.original_custody.record_digest[0] ^= 1,
        |c| c.original_custody.control_state_digest[0] ^= 1,
        |c| c.signatures_digest[0] ^= 1,
        |c| c.commitment.response_digest[0] ^= 1,
        |c| c.reservation.reservation_id[0] ^= 1,
        |c| c.reservation.fence += 1,
        |c| c.anchor.operation_state_digest = [0; 32],
    ];
    for mutate in completed_mutations {
        let mut evidence = checked_fixture(Phase::BeforeRelease);
        mutate(evidence.completed_mut());
        evidence.resign();
        assert_error(
            &mut evidence,
            EvidenceError::Receipt(ReceiptError::CompletionMismatch),
        );
    }
    let mut changed_control = checked_fixture(Phase::BeforeRelease);
    changed_control.state.body.current_anchor.height += 1;
    changed_control.state.body.current_anchor.block_hash = [0x94; 32];
    changed_control.state.body.current_anchor.state_digest[0] ^= 1;
    changed_control.resign();
    assert_error(
        &mut changed_control,
        EvidenceError::Receipt(ReceiptError::TokenMismatch),
    );
}

#[test]
fn completion_must_exist_at_observation_and_the_token_must_remain_live_at_release() {
    let make_early = || {
        let mut receipt = receipt_fixture::fixture();
        receipt.current.now_unix_ms = 1_140_000;
        receipt.current.anchor_observed_at_unix_ms = 1_131_000;
        let mut evidence = Evidence::with_receipt(Phase::BeforeRelease, receipt);
        assert_eq!(evidence.request.not_before_unix_ms, 1_130_000);
        assert_eq!(evidence.state.body.observed_at_unix_ms, 1_131_000);
        evidence.state.body.expires_at_unix_ms = 1_141_000;
        evidence.resign();
        evidence
    };
    let mut baseline = make_early();
    assert_positive(&mut baseline);
    let mut future_completion = make_early();
    future_completion.completed_mut().completed_at_unix_ms = 1_132_000;
    future_completion.resign();
    assert_error(&mut future_completion, EvidenceError::InvalidState);
    let make_expiring = || {
        let mut receipt = receipt_fixture::fixture();
        receipt.current.now_unix_ms = receipt.token.body.ttl_epoch * 1_000 - 1;
        receipt.current.anchor_observed_at_unix_ms = receipt.current.now_unix_ms;
        Evidence::with_receipt(Phase::BeforeRelease, receipt)
    };
    let mut before_expiry = make_expiring();
    assert_positive(&mut before_expiry);
    let mut at_expiry = make_expiring();
    at_expiry.now += 1;
    assert_error(
        &mut at_expiry,
        EvidenceError::Receipt(ReceiptError::TokenExpired),
    );
}

#[test]
fn phase_partition_nonzero_challenge_and_retained_floors_are_mandatory() {
    let baseline = checked_fixture(Phase::Startup);
    for phase in phases() {
        let current = SignerStreamTokenObservationExpectedV1::current(
            &baseline.receipt.binding,
            phase,
            [0x91; 32],
            baseline.request.minimum_anchor,
            baseline.request.not_before_unix_ms,
        );
        let completed = SignerStreamTokenObservationExpectedV1::completed(
            &baseline.receipt_bytes,
            &baseline.receipt.token,
            &baseline.receipt.expected,
            &baseline.receipt.binding,
            phase,
            [0x91; 32],
            baseline.request.minimum_anchor,
            baseline.request.not_before_unix_ms,
        );
        if is_current(phase) {
            assert!(current.is_ok());
            assert_eq!(completed.err(), Some(EvidenceError::SourceMismatch));
        } else {
            assert!(completed.is_ok());
            assert_eq!(current.err(), Some(EvidenceError::SourceMismatch));
        }
    }
    for invalid in 0..5 {
        let mut challenge = baseline.request.challenge;
        let mut floor = baseline.request.minimum_anchor;
        let mut not_before = baseline.request.not_before_unix_ms;
        match invalid {
            0 => challenge = [0; 32],
            1 => floor.height = 0,
            2 => floor.block_hash = [0; 32],
            3 => floor.state_digest = [0; 32],
            _ => not_before = 0,
        }
        assert_eq!(
            SignerStreamTokenObservationExpectedV1::current(
                &baseline.receipt.binding,
                Phase::Startup,
                challenge,
                floor,
                not_before
            )
            .err(),
            Some(EvidenceError::SourceMismatch)
        );
    }
    for (original, substituted) in [
        (Phase::Startup, Phase::BeforeRelease),
        (Phase::BeforeRelease, Phase::Startup),
        (Phase::BeforeAdmission, Phase::AfterCommit),
        (Phase::AfterCommit, Phase::BeforeAdmission),
        (Phase::BeforeAdmission, Phase::BeforeRelease),
        (Phase::BeforeRelease, Phase::BeforeAdmission),
        (Phase::AfterCommit, Phase::BeforeRelease),
        (Phase::BeforeRelease, Phase::AfterCommit),
    ] {
        let mut evidence = checked_fixture(original);
        evidence.phase = substituted; // Choose the other entry point while retaining the original attempt.
        assert_error(&mut evidence, EvidenceError::SourceMismatch);
    }
    for (original, substituted) in [
        (Phase::Startup, Phase::BeforeAdmission),
        (Phase::BeforeAdmission, Phase::Startup),
        (Phase::BeforeAdmission, Phase::BeforeProvider),
        (Phase::BeforeProvider, Phase::BeforeAdmission),
        (Phase::BeforeProvider, Phase::AfterProvider),
        (Phase::AfterProvider, Phase::BeforeCommit),
        (Phase::BeforeCommit, Phase::Startup),
        (Phase::AfterCommit, Phase::BeforeRelease),
    ] {
        let mut evidence = checked_fixture(original);
        evidence.state.body.phase = substituted;
        evidence.resign();
        assert_error(&mut evidence, EvidenceError::SourceMismatch);
    }
    for (original, substituted) in [
        (Phase::Startup, Phase::BeforeRelease),
        (Phase::BeforeRelease, Phase::Startup),
    ] {
        let other = checked_fixture(substituted);
        let mut evidence = checked_fixture(original);
        evidence.state.body.phase = substituted;
        evidence.state.body.subject = other.state.body.subject;
        evidence.resign();
        assert_error(&mut evidence, EvidenceError::SourceMismatch);
    }
    let invalid_request: &[fn(&mut SignerStreamTokenObservationRequestV1)] = &[
        |r| r.magic = *b"IRSTKQ00",
        |r| r.challenge = [0; 32],
        |r| r.minimum_anchor.height = 0,
        |r| r.not_before_unix_ms = 0,
        |r| {
            r.subject = SignerStreamTokenObservationRequestSubjectV1::CurrentCustody {
                binding_digest: [0; 32],
            }
        },
        |r| r.phase = Phase::AfterCommit,
    ];
    for mutate in invalid_request {
        let mut request = baseline.request.clone();
        mutate(&mut request);
        assert_eq!(
            request.encode_canonical(),
            Err(EvidenceError::SourceMismatch)
        );
        assert_eq!(request.digest(), Err(EvidenceError::SourceMismatch));
        assert_eq!(
            SignerStreamTokenObservationRequestV1::decode_canonical(
                &norito::encode_canonical(&request).expect("malformed semantic request")
            ),
            Err(EvidenceError::SourceMismatch)
        );
    }
}

#[test]
fn signed_subjects_and_completed_request_prevalidation_bind_exact_token_and_provider() {
    for field in 0..4 {
        let mut evidence = checked_fixture(Phase::BeforeRelease);
        let SignerStreamTokenStateSubjectV1::CompletedOperation {
            binding_digest,
            operation_id,
            signing_payload_digest,
            signing_payload_size,
            ..
        } = &mut evidence.state.body.subject
        else {
            panic!("completed subject")
        };
        match field {
            0 => binding_digest[0] ^= 1,
            1 => operation_id[0] ^= 1,
            2 => signing_payload_digest[0] ^= 1,
            _ => *signing_payload_size += 1,
        }
        evidence.resign();
        assert_error(&mut evidence, EvidenceError::SourceMismatch);
    }
    let mut current = checked_fixture(Phase::Startup);
    let SignerStreamTokenStateSubjectV1::CurrentCustody { binding_digest } =
        &mut current.state.body.subject
    else {
        panic!("current subject")
    };
    binding_digest[0] ^= 1;
    current.resign();
    assert_error(&mut current, EvidenceError::SourceMismatch);
    let mut changed_body = checked_fixture(Phase::BeforeRelease);
    changed_body.receipt.token.body.max_streams += 1;
    assert_error(
        &mut changed_body,
        EvidenceError::Receipt(ReceiptError::TokenMismatch),
    );
    let mut wrong_provider = checked_fixture(Phase::BeforeRelease);
    wrong_provider.receipt.token.body.provider_id[0] ^= 1;
    assert_error(
        &mut wrong_provider,
        EvidenceError::Receipt(ReceiptError::WrongPurpose),
    );
    let mut wrong_key = checked_fixture(Phase::BeforeRelease);
    wrong_key.receipt.binding.public_key = receipt_fixture::key(0x29).public_key().clone();
    assert_error(
        &mut wrong_key,
        EvidenceError::Receipt(ReceiptError::TokenMismatch),
    );
    let baseline = checked_fixture(Phase::BeforeRelease);
    let prepare = |bytes: &[u8], token: &StreamTokenV1| {
        SignerStreamTokenObservationExpectedV1::completed(
            bytes,
            token,
            &baseline.receipt.expected,
            &baseline.receipt.binding,
            Phase::BeforeRelease,
            [0x99; 32],
            baseline.request.minimum_anchor,
            baseline.request.not_before_unix_ms,
        )
    };
    let mut bare_signature = baseline.receipt.token.clone();
    bare_signature.signature[0] ^= 1;
    assert_eq!(
        prepare(&baseline.receipt_bytes, &bare_signature).err(),
        Some(EvidenceError::Receipt(ReceiptError::InvalidSignature))
    );
    assert_eq!(
        prepare(&baseline.receipt.token.signature, &baseline.receipt.token).err(),
        Some(EvidenceError::Receipt(ReceiptError::InvalidReceipt))
    );
    let mut missing = baseline.receipt.receipt.clone();
    missing.signatures.pop();
    assert_eq!(
        prepare(
            &norito::encode_canonical(&missing).expect("missing signature claim"),
            &baseline.receipt.token
        )
        .err(),
        Some(EvidenceError::Receipt(ReceiptError::InvalidSignature))
    );
    let mut unsupported = baseline.receipt.receipt.clone();
    unsupported.signatures[1].signature[0] ^= 1;
    let candidate = norito::encode_canonical(&unsupported)
        .expect("correct token but forged follow-on signature");
    prepare(&candidate, &baseline.receipt.token)
        .expect("limited prevalidation creates a query, not verified authority");
    let mut evidence = Evidence::new(Phase::BeforeRelease);
    evidence.attempt =
        Some(prepare(&candidate, &baseline.receipt.token).expect("prevalidated public claim"));
    evidence.request = evidence
        .attempt
        .as_ref()
        .expect("pending")
        .request()
        .clone();
    evidence.receipt_bytes = candidate;
    evidence.state.body.request_digest = request_digest(&evidence.request);
    evidence.resign();
    assert_error(
        &mut evidence,
        EvidenceError::Receipt(ReceiptError::InvalidSignature),
    );
}
