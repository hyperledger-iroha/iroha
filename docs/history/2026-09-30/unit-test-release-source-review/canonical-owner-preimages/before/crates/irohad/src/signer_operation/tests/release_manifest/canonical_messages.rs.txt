//! Private ceremony ownership and substitution controls using simulated custody/state providers.
//! These tests do not supply native finalized authority or qualify configured production signing.

use super::*;
use crate::signer_operation::release_manifest::ceremony::ReleaseManifestCeremonyV1;
use SignerKeyOperationPurposeV1::{AuditRecord, Provenance, Response, RolePayload};
use sorafs_manifest::signer::{
    protocol::{
        SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, SignerOperationSignatureV1,
        signer_operation_message_digest_v1,
    },
    receipt::{
        SignerOperationProvenanceV1, SignerReleaseManifestRequestV1,
        signer_release_manifest_audit_v1, signer_release_manifest_response_digest_v1,
    },
};

fn release_fixture() -> Fixture {
    fixture_for(
        SignerRoleV1::ReleaseManifest,
        SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into(),
        },
    )
}
fn admitted<'a>(
    fixture: &'a Fixture,
    bytes: &[u8],
    expected: &SignerReleaseManifestExpectedV1,
) -> (VerifiedSignerCustodyV1, SignerOperationV1<'a>) {
    let custody = fixture
        .coordinator
        .verify(
            &fixture
                .coordinator
                .source
                .observe(&fixture.coordinator.binding)
                .unwrap(),
        )
        .unwrap();
    let request = SignerReleaseManifestRequestV1::new(&custody, expected, bytes).unwrap();
    let intent = SignerOperationIntentV1 {
        action: SignerOperationActionV1::Sign,
        operation_id: expected.operation_id,
        request_digest: request.digest().unwrap(),
        previous_audit: intent(SignerOperationActionV1::Sign).previous_audit,
    };
    let operation = fixture.coordinator.begin(intent).unwrap();
    (custody, operation)
}
fn append(
    owner: &ReleaseManifestCeremonyV1<'_>,
    operation: &mut SignerOperationV1<'_>,
    prefix: &mut Vec<SignerOperationSignatureV1>,
    purpose: SignerKeyOperationPurposeV1,
    anchor: Option<SignerCustodyAnchorV1>,
) {
    let message = owner
        .message(&operation.check(), purpose, prefix, anchor)
        .unwrap();
    message.validate_operation(&operation.check()).unwrap();
    assert_eq!(usize::from(message.ordinal()), prefix.len() + 1);
    let signature = operation.sign(message.purpose(), message.bytes()).unwrap();
    prefix.push(SignerOperationSignatureV1 {
        purpose,
        message_digest: signer_operation_message_digest_v1(message.bytes()),
        signature: signature.to_vec(),
    });
}

#[test]
fn owner_derives_the_existing_four_message_contract_across_finality_advance() {
    let fixture = release_fixture();
    let reviewed = expected();
    let (custody, mut operation) = admitted(&fixture, manifest(), &reviewed);
    let request = SignerReleaseManifestRequestV1::new(&custody, &reviewed, manifest()).unwrap();
    let owner = ReleaseManifestCeremonyV1::new(
        &fixture.source.binding,
        &reviewed,
        manifest(),
        &custody,
        &operation.check(),
    )
    .unwrap();
    let role = owner
        .message(&operation.check(), RolePayload, &[], None)
        .unwrap();
    assert_eq!(role.bytes(), manifest());
    let mut prefix = Vec::new();
    append(&owner, &mut operation, &mut prefix, RolePayload, None);
    let expected_audit = signer_release_manifest_audit_v1(
        &request,
        &operation.intent,
        operation.reservation,
        &prefix[0].signature,
    )
    .unwrap();
    let audit = owner
        .message(&operation.check(), AuditRecord, &prefix, None)
        .unwrap();
    assert_eq!(audit.bytes(), expected_audit.signing_message());
    *fixture.provider.fault.lock().unwrap() = Some(ProviderFault::AdvanceFinality);
    append(&owner, &mut operation, &mut prefix, AuditRecord, None);
    let anchor = operation.custody.current_anchor();
    assert!(anchor.height > custody.current_anchor().height);
    let expected_provenance = SignerOperationProvenanceV1 {
        original_custody: request.original_custody,
        signing_anchor: anchor,
        intent_digest: operation.intent_digest,
        reservation: operation.reservation,
        audit: expected_audit,
    };
    let provenance = owner
        .message(&operation.check(), Provenance, &prefix, Some(anchor))
        .unwrap();
    assert_eq!(
        provenance.bytes(),
        expected_provenance.signing_message().unwrap()
    );
    *fixture.provider.fault.lock().unwrap() = Some(ProviderFault::AdvanceFinality);
    append(
        &owner,
        &mut operation,
        &mut prefix,
        Provenance,
        Some(anchor),
    );
    let expected_commitment = SignerOperationCommitmentV1 {
        audit: expected_audit,
        response_digest: signer_release_manifest_response_digest_v1(
            &request,
            &expected_provenance,
            &prefix,
        )
        .unwrap(),
    };
    let response = owner
        .message(&operation.check(), Response, &prefix, Some(anchor))
        .unwrap();
    assert_eq!(
        response.bytes(),
        expected_commitment.response_signing_message()
    );
    append(&owner, &mut operation, &mut prefix, Response, Some(anchor));
    assert_eq!(
        owner
            .completion(&operation.check(), &prefix, anchor)
            .unwrap(),
        (expected_provenance, expected_commitment)
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 4);
    assert_eq!(
        fixture.source.state.lock().unwrap().commits,
        0,
        "canonical messages alone cannot complete or release a receipt"
    );
}

#[test]
fn owner_rejects_full_binding_and_reviewed_subject_substitutions_before_io() {
    let fixture = release_fixture();
    let reviewed = expected();
    let (custody, operation) = admitted(&fixture, manifest(), &reviewed);
    for variant in 0..6 {
        let mut binding = fixture.source.binding.clone();
        match variant {
            0 => binding.key_handle.push_str("-other"),
            1 => binding.key_revision += 1,
            2 => binding.policy_digest[0] ^= 1,
            3 => binding.chain_id = "another-chain".parse().unwrap(),
            4 => binding.runtime_handle.push_str("-other"),
            _ => binding.purpose = SignerPurposeBindingV1::NativeOrPromotion,
        }
        assert!(
            ReleaseManifestCeremonyV1::new(
                &binding,
                &reviewed,
                manifest(),
                &custody,
                &operation.check()
            )
            .is_err()
        );
    }
    for variant in 0..4 {
        let mut changed = reviewed;
        match variant {
            0 => changed.manifest_digest[0] ^= 1,
            1 => changed.manifest_size += 1,
            2 => changed.operation_id[0] ^= 1,
            _ => changed.operation_id = [0; 32],
        }
        assert!(
            ReleaseManifestCeremonyV1::new(
                &fixture.source.binding,
                &changed,
                manifest(),
                &custody,
                &operation.check()
            )
            .is_err()
        );
    }
    assert!(
        ReleaseManifestCeremonyV1::new(
            &fixture.source.binding,
            &reviewed,
            b"substituted",
            &custody,
            &operation.check()
        )
        .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn owner_accepts_exact_manifest_ceiling_and_rejects_empty_and_over_limit() {
    let fixture = release_fixture();
    let bytes = vec![0x61; SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1];
    let reviewed = SignerReleaseManifestExpectedV1 {
        operation_id: expected().operation_id,
        manifest_digest: signer_release_manifest_digest_v1(&bytes),
        manifest_size: bytes.len() as u64,
    };
    let (custody, operation) = admitted(&fixture, &bytes, &reviewed);
    let owner = ReleaseManifestCeremonyV1::new(
        &fixture.source.binding,
        &reviewed,
        &bytes,
        &custody,
        &operation.check(),
    )
    .unwrap();
    assert_eq!(
        owner
            .message(&operation.check(), RolePayload, &[], None)
            .unwrap()
            .bytes(),
        bytes
    );
    for invalid in [
        Vec::new(),
        vec![0x61; SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1 + 1],
    ] {
        let expected = SignerReleaseManifestExpectedV1 {
            manifest_digest: signer_release_manifest_digest_v1(&invalid),
            manifest_size: invalid.len() as u64,
            ..reviewed
        };
        assert!(
            ReleaseManifestCeremonyV1::new(
                &fixture.source.binding,
                &expected,
                &invalid,
                &custody,
                &operation.check()
            )
            .is_err()
        );
    }
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn prepared_message_rejects_another_intent_or_reservation_before_io() {
    let fixture = release_fixture();
    let reviewed = expected();
    let (custody, operation) = admitted(&fixture, manifest(), &reviewed);
    let owner = ReleaseManifestCeremonyV1::new(
        &fixture.source.binding,
        &reviewed,
        manifest(),
        &custody,
        &operation.check(),
    )
    .unwrap();
    let prepared = owner
        .message(&operation.check(), RolePayload, &[], None)
        .unwrap();
    for variant in 0..7 {
        let mut intent = operation.intent;
        let mut reservation = operation.reservation;
        match variant {
            0 => intent.operation_id[0] ^= 1,
            1 => intent.request_digest[0] ^= 1,
            2 => intent.action = SignerOperationActionV1::Qualify,
            3 => intent.previous_audit.digest[0] ^= 1,
            4 => reservation.reservation_id[0] ^= 1,
            5 => reservation.fence += 1,
            _ => reservation.expires_at_unix_ms -= 1,
        }
        let check = reservation_check(
            &intent,
            intent.digest().unwrap(),
            &operation.custody,
            reservation,
        );
        assert!(prepared.validate_operation(&check).is_err());
    }
    let forged_digest = reservation_check(
        &operation.intent,
        [0xFE; 32],
        &operation.custody,
        operation.reservation,
    );
    assert!(prepared.validate_operation(&forged_digest).is_err());
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn owner_rejects_wrong_purpose_prefix_signature_and_provenance_anchor() {
    let fixture = release_fixture();
    let reviewed = expected();
    let (custody, mut operation) = admitted(&fixture, manifest(), &reviewed);
    let owner = ReleaseManifestCeremonyV1::new(
        &fixture.source.binding,
        &reviewed,
        manifest(),
        &custody,
        &operation.check(),
    )
    .unwrap();
    assert!(
        owner
            .message(&operation.check(), AuditRecord, &[], None)
            .is_err()
    );
    let mut prefix = Vec::new();
    append(&owner, &mut operation, &mut prefix, RolePayload, None);
    for variant in 0..5 {
        let mut hostile = prefix.clone();
        match variant {
            0 => hostile[0].purpose = Response,
            1 => hostile[0].message_digest[0] ^= 1,
            2 => hostile[0].signature[0] ^= 1,
            3 => {
                hostile[0].signature = Signature::try_new(key(99).private_key(), manifest())
                    .unwrap()
                    .payload()
                    .to_vec()
            }
            _ => {
                hostile[0].signature.pop();
            }
        }
        assert!(
            owner
                .message(&operation.check(), AuditRecord, &hostile, None)
                .is_err()
        );
    }
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 1);
    append(&owner, &mut operation, &mut prefix, AuditRecord, None);
    let anchor = operation.custody.current_anchor();
    let mut reordered = prefix.clone();
    reordered.swap(0, 1);
    assert!(
        owner
            .message(&operation.check(), Provenance, &reordered, Some(anchor))
            .is_err()
    );
    for variant in 0..4 {
        let mut changed = anchor;
        match variant {
            0 => changed.block_hash[0] ^= 1,
            1 => changed.height += 1,
            2 => changed.height -= 1,
            _ => changed.state_digest[0] ^= 1,
        }
        assert!(
            owner
                .message(&operation.check(), Provenance, &prefix, Some(changed))
                .is_err()
        );
    }
    append(
        &owner,
        &mut operation,
        &mut prefix,
        Provenance,
        Some(anchor),
    );
    let mut changed = anchor;
    changed.block_hash[0] ^= 1;
    assert!(
        owner
            .message(&operation.check(), Response, &prefix, Some(changed))
            .is_err()
    );
    append(&owner, &mut operation, &mut prefix, Response, Some(anchor));
    assert!(
        owner
            .message(&operation.check(), RolePayload, &prefix, Some(anchor))
            .is_err()
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn owner_rejects_oversized_prefix_and_incomplete_or_mutated_completion() {
    let fixture = release_fixture();
    let reviewed = expected();
    let (custody, mut operation) = admitted(&fixture, manifest(), &reviewed);
    let owner = ReleaseManifestCeremonyV1::new(
        &fixture.source.binding,
        &reviewed,
        manifest(),
        &custody,
        &operation.check(),
    )
    .unwrap();
    let anchor = operation.custody.current_anchor();
    let mut prefix = Vec::new();
    for purpose in [RolePayload, AuditRecord, Provenance, Response] {
        let signing_anchor = (prefix.len() >= 2).then_some(anchor);
        append(&owner, &mut operation, &mut prefix, purpose, signing_anchor);
    }
    let mut excessive = prefix.clone();
    excessive.push(prefix[0].clone());
    assert!(
        owner
            .message(&operation.check(), RolePayload, &excessive, Some(anchor))
            .is_err()
    );
    for variant in 0..5 {
        let fresh_owner = ReleaseManifestCeremonyV1::new(
            &fixture.source.binding,
            &reviewed,
            manifest(),
            &custody,
            &operation.check(),
        )
        .unwrap();
        let mut hostile = prefix.clone();
        if variant == 4 {
            hostile.pop();
        } else {
            hostile[variant].signature[0] ^= 1;
        }
        assert!(
            fresh_owner
                .completion(&operation.check(), &hostile, anchor)
                .is_err()
        );
    }
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn release_caller_keeps_key_substitution_expiry_and_revocation_failures_unreleased() {
    for fault in [
        ProviderFault::WrongKey,
        ProviderFault::WrongMessage,
        ProviderFault::Mutate(Mutation::Expired),
        ProviderFault::Mutate(Mutation::SignerRevoked),
        ProviderFault::Mutate(Mutation::AttesterRevoked),
        ProviderFault::Unavailable,
    ] {
        let directory = private_directory();
        let canonical = directory.path().canonicalize().unwrap();
        let (service, source, provider) = ceremony(&canonical);
        *provider.fault.lock().unwrap() = Some(fault);
        assert!(service.sign(manifest()).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert!(
            source
                .state
                .lock()
                .unwrap()
                .used_ids
                .contains(&expected().operation_id)
        );
        assert!(source.state.lock().unwrap().completed.is_none());
        assert_eq!(fs::read_dir(&canonical).unwrap().count(), 0);
        assert!(service.recover(manifest()).is_err());
        assert!(service.sign(manifest()).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }
}
