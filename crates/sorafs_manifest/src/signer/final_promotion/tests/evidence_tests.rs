//! Independent source, observer and complete receipt regressions; no deployment qualification.

use super::*;
use crate::signer::{
    final_promotion::tests::fixtures::EvidenceFixture, receipt::SignerReceiptErrorV1,
};

#[test]
fn signed_finalized_state_authenticates_the_complete_exact_receipt() {
    let evidence = EvidenceFixture::new();
    let verified = evidence.verify().unwrap();
    assert_eq!(verified.completion(), &evidence.receipt.completion);
    assert_eq!(
        verified.statement_digest(),
        evidence.receipt.expected.statement_digest
    );
}

#[test]
fn source_pins_cannot_be_substituted_with_candidate_policy_or_trust() {
    let mut evidence = EvidenceFixture::new();
    evidence.policy.operation_id[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::SourceMismatch
    );
    let mut evidence = EvidenceFixture::new();
    evidence.trust.state_public_key = evidence.receipt.signer.public_key().clone();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::SourceMismatch
    );
    let mut evidence = EvidenceFixture::new();
    evidence.receipt.message[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::SourceMismatch
    );
}

#[test]
fn independently_pinned_trust_still_rejects_shared_keys_and_administrators() {
    for mutation in 0..4 {
        let mut evidence = EvidenceFixture::new();
        match mutation {
            0 => evidence.trust.state_public_key = evidence.receipt.signer.public_key().clone(),
            1 => evidence.trust.state_public_key = evidence.receipt.attester.public_key().clone(),
            2 => {
                evidence.trust.state_authority.administrator_id =
                    evidence.policy.binding.administrator_id.clone()
            }
            _ => {
                evidence.trust.state_authority.administrator_id =
                    evidence.trust.custody_authority.administrator_id.clone()
            }
        }
        evidence.expected.trust_sha256 = sha256(norito::encode_canonical(&evidence.trust).unwrap());
        assert_eq!(
            evidence.verify().unwrap_err(),
            SignerFinalPromotionEvidenceErrorV1::InvalidTrust
        );
    }
}

#[test]
fn observer_identities_are_disjoint_in_every_service_and_administrator_slot() {
    for slot in 0..2 {
        for other in 0..5 {
            let mut evidence = EvidenceFixture::new();
            let reused = match other {
                0 => evidence.policy.binding.service_id.clone(),
                1 => evidence.policy.binding.administrator_id.clone(),
                2 => evidence.trust.custody_authority.service_id.clone(),
                3 => evidence.trust.custody_authority.administrator_id.clone(),
                _ if slot == 0 => evidence.trust.state_authority.administrator_id.clone(),
                _ => evidence.trust.state_authority.service_id.clone(),
            };
            if slot == 0 {
                evidence.trust.state_authority.service_id = reused;
            } else {
                evidence.trust.state_authority.administrator_id = reused;
            }
            evidence.expected.trust_sha256 =
                sha256(norito::encode_canonical(&evidence.trust).unwrap());
            evidence.state.body.authority = evidence.trust.state_authority.clone();
            evidence.sign_state();
            assert_eq!(
                evidence.verify().unwrap_err(),
                SignerFinalPromotionEvidenceErrorV1::InvalidTrust,
                "observer slot {slot}, reused identity {other}"
            );
        }
    }
}

#[test]
fn signed_state_rejects_stale_future_revoked_and_forked_observations() {
    for mutation in 0..7 {
        let mut evidence = EvidenceFixture::new();
        match mutation {
            0 => evidence.expected.now_unix_ms = evidence.state.body.expires_at_unix_ms,
            1 => evidence.expected.now_unix_ms = evidence.state.body.observed_at_unix_ms - 1,
            2 => evidence.state.body.signer_revoked = true,
            3 => evidence.state.body.attester_revoked = true,
            4 => evidence.state.body.current_anchor.block_hash[0] ^= 1,
            5 => evidence.state.body.current_anchor.height -= 1,
            _ => evidence.state.body.current_anchor.state_digest[0] ^= 1,
        }
        evidence.sign_state();
        assert_eq!(
            evidence.verify().unwrap_err(),
            SignerFinalPromotionEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn real_observer_signature_cannot_move_between_networks_deployments_or_policy() {
    for mutation in 0..4 {
        let mut evidence = EvidenceFixture::new();
        match mutation {
            0 => evidence.state.body.network_id[0] ^= 1,
            1 => evidence.state.body.deployment_id = "another-deployment".into(),
            2 => evidence.state.body.reviewed_policy_sha256[0] ^= 1,
            _ => evidence.state.body.authority.key_revision += 1,
        }
        evidence.sign_state();
        assert_eq!(
            evidence.verify().unwrap_err(),
            SignerFinalPromotionEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn changed_observation_requires_the_independent_observer_signature() {
    let mut evidence = EvidenceFixture::new();
    evidence
        .state
        .body
        .completed_operation
        .commitment
        .response_digest[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::InvalidState
    );
    evidence.sign_state();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::Receipt(
            SignerFinalPromotionReceiptErrorV1::Operation(SignerReceiptErrorV1::CompletionMismatch)
        )
    );
}

#[test]
fn an_observer_cannot_authenticate_completion_before_it_happened() {
    let mut evidence = EvidenceFixture::new();
    evidence.state.body.observed_at_unix_ms = 124_999;
    evidence.state.body.expires_at_unix_ms = 129_999;
    evidence.expected.now_unix_ms = 125_000;
    evidence.sign_state();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::InvalidState
    );
}

#[test]
fn observation_signing_bounds_every_variable_field_before_serialization() {
    for field in 0..4 {
        let mut evidence = EvidenceFixture::new();
        let oversized = "x".repeat(SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1 + 1);
        match field {
            0 => evidence.state.body.chain_id = oversized,
            1 => evidence.state.body.deployment_id = oversized,
            2 => evidence.state.body.authority.service_id = oversized,
            _ => evidence.state.body.authority.administrator_id = oversized,
        }
        assert_eq!(
            evidence.state.body.signing_payload().unwrap_err(),
            SignerFinalPromotionEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn substituted_active_custody_and_raw_manifest_signature_fail_after_state_authentication() {
    let mut evidence = EvidenceFixture::new();
    evidence.state.body.active_head.record_digest[0] ^= 1;
    evidence.sign_state();
    assert!(matches!(
        evidence.verify(),
        Err(SignerFinalPromotionEvidenceErrorV1::Receipt(
            SignerFinalPromotionReceiptErrorV1::Custody(_)
        ))
    ));
    let mut evidence = EvidenceFixture::new();
    evidence.receipt.receipt.signatures[0].signature[0] ^= 1;
    assert!(matches!(
        evidence.verify(),
        Err(SignerFinalPromotionEvidenceErrorV1::Receipt(_))
    ));
}

#[test]
fn evidence_decoding_is_bounded_and_rejects_alternate_headers() {
    let evidence = EvidenceFixture::new();
    let encoded = norito::encode_canonical(&evidence.state).unwrap();
    assert!(decode::<SignerFinalPromotionStateObservationV1>(&encoded).is_ok());
    assert_eq!(
        decode::<SignerFinalPromotionStateObservationV1>(&vec![
            0;
            SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1
                + 1
        ])
        .unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::InvalidDocument
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode::<SignerFinalPromotionStateObservationV1>(&trailing).unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::InvalidDocument
    );
    let changed = crate::canonical_test_support::with_compression_tag(&evidence.state);
    assert_eq!(
        decode::<SignerFinalPromotionStateObservationV1>(&changed).unwrap_err(),
        SignerFinalPromotionEvidenceErrorV1::InvalidDocument
    );
}

#[test]
fn signed_observation_must_bind_the_exact_statement_hash_and_size() {
    for mutation in 0..2 {
        let mut evidence = EvidenceFixture::new();
        match mutation {
            0 => evidence.state.body.statement_sha256[0] ^= 1,
            _ => evidence.state.body.statement_size += 1,
        }
        evidence.sign_state();
        assert_eq!(
            evidence.verify().unwrap_err(),
            SignerFinalPromotionEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn all_public_evidence_frames_are_purpose_owned_and_bounded() {
    let evidence = EvidenceFixture::new();
    let policy_bytes = evidence.policy_bytes();
    let trust_bytes = evidence.trust_bytes();
    let state_bytes = evidence.state_bytes();
    let receipt_bytes = evidence.receipt_bytes();
    assert!(
        norito::decode_canonical::<crate::signer::release_evidence::SignerReleaseEvidencePolicyV1>(
            &policy_bytes
        )
        .is_err()
    );
    assert!(
        norito::decode_canonical::<crate::signer::release_evidence::SignerReleaseEvidenceTrustV1>(
            &trust_bytes
        )
        .is_err()
    );
    assert!(norito::decode_canonical::<crate::signer::release_evidence::SignerReleaseStateObservationV1>(&state_bytes).is_err());
    assert!(
        norito::decode_canonical::<crate::signer::receipt::SignerReleaseManifestReceiptV1>(
            &receipt_bytes
        )
        .is_err()
    );
    let raw_key = evidence.raw_public_key();
    let inputs = [policy_bytes, trust_bytes, state_bytes, receipt_bytes];
    for slot in 0..4 {
        for oversized in [false, true] {
            let mut replaced = inputs.clone();
            replaced[slot] = if oversized {
                vec![0; SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1 + 1]
            } else {
                Vec::new()
            };
            assert!(
                verify_final_promotion_evidence_v1(
                    &replaced[0],
                    &replaced[1],
                    &replaced[2],
                    &replaced[3],
                    &evidence.receipt.message,
                    &evidence.receipt.receipt.signatures[0].signature,
                    &raw_key,
                    &evidence.expected
                )
                .is_err()
            );
        }
    }
}
