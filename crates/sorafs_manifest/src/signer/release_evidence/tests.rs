//! Full signed receipt simulations; these do not qualify a deployed hardware provider.

use super::*;
use crate::signer::receipt::tests::{Fixture, fixture};
use iroha_crypto::KeyPair;

struct Evidence {
    receipt: Fixture,
    observer: KeyPair,
    policy: SignerReleaseEvidencePolicyV1,
    trust: SignerReleaseEvidenceTrustV1,
    state: SignerReleaseStateObservationV1,
    expected: SignerReleaseEvidenceExpectedV1,
}
impl Evidence {
    fn new() -> Self {
        let receipt = fixture();
        let observer = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).unwrap();
        let policy = SignerReleaseEvidencePolicyV1 {
            magic: SignerReleaseEvidencePolicyV1::magic(),
            binding: receipt.binding.clone(),
            operation_id: receipt.expected.operation_id,
            manifest_sha256: sha256(&receipt.manifest),
            manifest_size: receipt.expected.manifest_size,
            minimum_anchor: receipt.current.current_anchor,
        };
        let trust = SignerReleaseEvidenceTrustV1 {
            magic: SignerReleaseEvidenceTrustV1::magic(),
            custody_authority: receipt.trust.authority.clone(),
            custody_public_key: receipt.trust.public_key.clone(),
            custody_active_from_unix_ms: receipt.trust.active_from_unix_ms,
            custody_active_until_unix_ms: receipt.trust.active_until_unix_ms,
            custody_max_validity_ms: receipt.trust.max_validity_ms,
            state_authority: SignerCustodyAuthorityV1 {
                service_id: "finalized-state-observer".into(),
                administrator_id: "independent-state-reviewer".into(),
                key_revision: 2,
                policy_revision: 4,
                policy_digest: [0x74; 32],
            },
            state_public_key: observer.public_key().clone(),
            state_active_from_unix_ms: 90_000,
            state_active_until_unix_ms: 300_000,
            max_state_age_ms: 10_000,
        };
        let expected = SignerReleaseEvidenceExpectedV1 {
            policy_sha256: sha256(norito::encode_canonical(&policy).unwrap()),
            trust_sha256: sha256(norito::encode_canonical(&trust).unwrap()),
            public_key_fingerprint_sha256: sha256(receipt.signer.public_key().to_bytes().1),
            now_unix_ms: receipt.current.now_unix_ms,
        };
        let state = SignerReleaseStateObservationV1 {
            body: SignerReleaseStateObservationBodyV1 {
                magic: SignerReleaseStateObservationBodyV1::magic(),
                reviewed_policy_sha256: expected.policy_sha256,
                authority: trust.state_authority.clone(),
                chain_id: policy.binding.chain_id.clone(),
                network_id: policy.binding.network_id,
                deployment_id: "production-primary".into(),
                observed_at_unix_ms: receipt.current.anchor_observed_at_unix_ms,
                expires_at_unix_ms: receipt.current.now_unix_ms + 5_000,
                current_anchor: receipt.current.current_anchor,
                active_head: receipt.current.active_head,
                signer_revoked: false,
                attester_revoked: false,
                completed_operation: receipt.completion,
            },
            signature: [0; 64],
        };
        let mut result = Self {
            receipt,
            observer,
            policy,
            trust,
            state,
            expected,
        };
        result.sign_state();
        result
    }
    fn sign_state(&mut self) {
        self.state.signature = Signature::new(
            self.observer.private_key(),
            &self.state.body.signing_payload().unwrap(),
        )
        .payload()
        .try_into()
        .unwrap();
    }
    fn verify(
        &self,
    ) -> Result<VerifiedReleaseManifestSignerReceiptV1, SignerReleaseEvidenceErrorV1> {
        verify_release_manifest_evidence_v1(
            &norito::encode_canonical(&self.policy).unwrap(),
            &norito::encode_canonical(&self.trust).unwrap(),
            &norito::encode_canonical(&self.state).unwrap(),
            &norito::encode_canonical(&self.receipt.receipt).unwrap(),
            &self.receipt.manifest,
            &self.receipt.receipt.signatures[0].signature,
            &self
                .receipt
                .signer
                .public_key()
                .to_bytes()
                .1
                .try_into()
                .unwrap(),
            &self.expected,
        )
    }
}

#[test]
fn signed_finalized_state_authenticates_the_complete_exact_receipt() {
    let evidence = Evidence::new();
    let verified = evidence.verify().unwrap();
    assert_eq!(verified.completion(), &evidence.receipt.completion);
    assert_eq!(
        verified.manifest_digest(),
        evidence.receipt.expected.manifest_digest
    );
}

#[test]
fn source_pins_cannot_be_substituted_with_candidate_policy_or_trust() {
    let mut evidence = Evidence::new();
    evidence.policy.operation_id[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::SourceMismatch
    );
    let mut evidence = Evidence::new();
    evidence.trust.state_public_key = evidence.receipt.signer.public_key().clone();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::SourceMismatch
    );
    let mut evidence = Evidence::new();
    evidence.receipt.manifest[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::SourceMismatch
    );
}

#[test]
fn independently_pinned_trust_still_rejects_shared_keys_and_administrators() {
    for mutation in 0..4 {
        let mut evidence = Evidence::new();
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
            SignerReleaseEvidenceErrorV1::InvalidTrust
        );
    }
}

#[test]
fn observer_identities_are_disjoint_in_every_service_and_administrator_slot() {
    for slot in 0..2 {
        for other in 0..5 {
            let mut evidence = Evidence::new();
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
                SignerReleaseEvidenceErrorV1::InvalidTrust,
                "observer slot {slot}, reused identity {other}"
            );
        }
    }
}

#[test]
fn signed_state_rejects_stale_future_revoked_and_forked_observations() {
    for mutation in 0..7 {
        let mut evidence = Evidence::new();
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
            SignerReleaseEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn real_observer_signature_cannot_move_between_networks_deployments_or_policy() {
    for mutation in 0..4 {
        let mut evidence = Evidence::new();
        match mutation {
            0 => evidence.state.body.network_id[0] ^= 1,
            1 => evidence.state.body.deployment_id = "another-deployment".into(),
            2 => evidence.state.body.reviewed_policy_sha256[0] ^= 1,
            _ => evidence.state.body.authority.key_revision += 1,
        }
        evidence.sign_state();
        assert_eq!(
            evidence.verify().unwrap_err(),
            SignerReleaseEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn changed_observation_requires_the_independent_observer_signature() {
    let mut evidence = Evidence::new();
    evidence
        .state
        .body
        .completed_operation
        .commitment
        .response_digest[0] ^= 1;
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::InvalidState
    );
    evidence.sign_state();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::Receipt(SignerReceiptErrorV1::CompletionMismatch)
    );
}

#[test]
fn an_observer_cannot_authenticate_completion_before_it_happened() {
    let mut evidence = Evidence::new();
    evidence.state.body.observed_at_unix_ms = 124_999;
    evidence.state.body.expires_at_unix_ms = 129_999;
    evidence.expected.now_unix_ms = 125_000;
    evidence.sign_state();
    assert_eq!(
        evidence.verify().unwrap_err(),
        SignerReleaseEvidenceErrorV1::InvalidState
    );
}

#[test]
fn observation_signing_bounds_every_variable_field_before_serialization() {
    for field in 0..4 {
        let mut evidence = Evidence::new();
        let oversized = "x".repeat(SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1 + 1);
        match field {
            0 => evidence.state.body.chain_id = oversized,
            1 => evidence.state.body.deployment_id = oversized,
            2 => evidence.state.body.authority.service_id = oversized,
            _ => evidence.state.body.authority.administrator_id = oversized,
        }
        assert_eq!(
            evidence.state.body.signing_payload().unwrap_err(),
            SignerReleaseEvidenceErrorV1::InvalidState
        );
    }
}

#[test]
fn substituted_active_custody_and_raw_manifest_signature_fail_after_state_authentication() {
    let mut evidence = Evidence::new();
    evidence.state.body.active_head.record_digest[0] ^= 1;
    evidence.sign_state();
    assert!(matches!(
        evidence.verify(),
        Err(SignerReleaseEvidenceErrorV1::Receipt(
            SignerReceiptErrorV1::Custody(_)
        ))
    ));
    let mut evidence = Evidence::new();
    evidence.receipt.receipt.signatures[0].signature[0] ^= 1;
    assert!(matches!(
        evidence.verify(),
        Err(SignerReleaseEvidenceErrorV1::Receipt(_))
    ));
}

#[test]
fn evidence_decoding_is_bounded_and_rejects_alternate_headers() {
    let evidence = Evidence::new();
    let encoded = norito::encode_canonical(&evidence.state).unwrap();
    assert!(decode::<SignerReleaseStateObservationV1>(&encoded).is_ok());
    assert_eq!(
        decode::<SignerReleaseStateObservationV1>(&vec![
            0;
            SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1
                + 1
        ])
        .unwrap_err(),
        SignerReleaseEvidenceErrorV1::InvalidDocument
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode::<SignerReleaseStateObservationV1>(&trailing).unwrap_err(),
        SignerReleaseEvidenceErrorV1::InvalidDocument
    );
    let changed = crate::canonical_test_support::with_compression_tag(&evidence.state);
    assert_eq!(
        decode::<SignerReleaseStateObservationV1>(&changed).unwrap_err(),
        SignerReleaseEvidenceErrorV1::InvalidDocument
    );
}
