//! Exact request retention and issuer projection checks using real synthetic Ed/BLS proofs.
//! These tests construct neither FI customer admission nor installed signer custody.
use super::*;
use crate::participant_enrollment_dispatch::{
    ParticipantEnrollmentIssuerPurposeV1, ParticipantIssuerPreparationProjectionV1,
    RetainedParticipantEnrollmentRequestV1,
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    kagemusha::{KagemushaRetailEnrollmentIssuerPolicyV1, KagemushaRetailEnrollmentRuntimeV1},
    nexus::AxtAssetIncarnationV1,
};
use iroha_model_base::topology::DataSpaceId;

fn verified(
    operation: ParticipantEnrollmentOperationV1,
    body: &[u8],
) -> VerifiedParticipantEnrollmentRequestV1 {
    let f = Fixture::new();
    let network = f.native.network_id();
    let target = Url::parse(&format!(
        "https://fi.example.invalid/leumi.is2{}",
        operation.path_suffix()
    ))
    .unwrap();
    let mut req = request(&f, &network, &target, body);
    req.operation = operation;
    let challenge = EnrollmentWalletReadChallengeV1::for_request(&req).unwrap();
    let nodes = Fixture::nodes();
    let statements = f.statements(challenge.bytes(), &nodes);
    let wallet = f.admit(challenge, &nodes, &statements).unwrap();
    let signature = Signature::new(f.signer.private_key(), &req.signing_message().unwrap());
    wallet.verify_request(&req, &signature).unwrap()
}
fn policy(network: NetworkId) -> KagemushaRetailEnrollmentIssuerPolicyV1 {
    KagemushaRetailEnrollmentIssuerPolicyV1 {
        version: 1,
        issuer_policy_id: [17; 32],
        issuer_public_key: KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
        issuer_audience: "fixture-ordinary-issuer".parse().unwrap(),
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "leumi.is2".parse().unwrap(),
            ledger_dataspace_id: DataSpaceId::new(10),
            authentication_namespace: "leumi.is2".parse().unwrap(),
            network_id: network,
            asset: AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"explicit issuer dispatch fixture incarnation").as_ref(),
            )
            .unwrap(),
            scale: 2,
        },
        valid_from_ms: 900_000,
        expires_at_ms: 2_000_000,
        maximum_certificate_lifetime_ms: 100_000,
    }
}

#[test]
fn retained_original_request_owns_received_bytes_and_preserves_native_subject() {
    let offered = b"{ \"exact\":\"original\" }\n".to_vec();
    let admitted = verified(ParticipantEnrollmentOperationV1::Prepare, &offered);
    let original_subject = admitted.request_sha256();
    let s = admitted.signatory().clone();
    let w = admitted.wallet().clone();
    let retained =
        RetainedParticipantEnrollmentRequestV1::retain(admitted, offered.clone()).unwrap();
    drop(offered);
    let dispatch = retained.dispatch().unwrap();
    assert_eq!(
        retained.original_body().unwrap(),
        b"{ \"exact\":\"original\" }\n"
    );
    assert_eq!(
        dispatch.original_body().unwrap(),
        retained.original_body().unwrap()
    );
    assert_eq!(dispatch.request().request_sha256(), original_subject);
    assert_eq!(dispatch.request().signatory(), &s);
    assert_eq!(dispatch.request().wallet(), &w);
    assert_eq!(dispatch.request().actor_id(), "fixture-retail-actor");
    assert_eq!(dispatch.request().namespace(), "leumi.is2");
    assert_eq!(dispatch.request().request_id(), "fixture-request");
    assert_eq!(
        dispatch.request().idempotency_key(),
        "fixture-stable-attempt"
    );
}

#[test]
fn retained_purpose_uses_the_actual_signature_and_never_sends_c_to_the_worker() {
    for (operation, purpose, phase) in [
        (
            ParticipantEnrollmentOperationV1::Prepare,
            ParticipantEnrollmentIssuerPurposeV1::NativePreparation,
            None,
        ),
        (
            ParticipantEnrollmentOperationV1::RawAttestation,
            ParticipantEnrollmentIssuerPurposeV1::RawAttestation,
            Some("raw"),
        ),
        (
            ParticipantEnrollmentOperationV1::Certificate,
            ParticipantEnrollmentIssuerPurposeV1::Credential,
            Some("credential"),
        ),
    ] {
        let retained = RetainedParticipantEnrollmentRequestV1::retain(
            verified(operation, b"original"),
            b"original".to_vec(),
        )
        .unwrap();
        let dispatch = retained.dispatch().unwrap();
        assert_eq!(dispatch.purpose(), purpose);
        assert_eq!(dispatch.purpose().worker_phase(), phase);
        assert_eq!(dispatch.request().operation(), operation);
    }
}

#[test]
fn retained_request_refuses_changed_body_bounds_and_expired_actual_proof() {
    for body in [
        b"changed".to_vec(),
        Vec::new(),
        vec![1; MAX_PARTICIPANT_ENROLLMENT_BODY_BYTES + 1],
    ] {
        assert!(
            RetainedParticipantEnrollmentRequestV1::retain(
                verified(ParticipantEnrollmentOperationV1::Prepare, b"original"),
                body
            )
            .is_err()
        );
    }
    let mut admitted = verified(ParticipantEnrollmentOperationV1::Prepare, b"original");
    // Expire the same actual synthetic challenged proof without sleeping or minting a marker.
    admitted.wallet.challenge.deadline = Instant::now();
    assert!(
        RetainedParticipantEnrollmentRequestV1::retain(admitted, b"original".to_vec()).is_err()
    );
}

#[test]
fn issuer_projection_uses_exact_native_ed_policy_and_rejects_other_role_or_policy() {
    let admitted = verified(ParticipantEnrollmentOperationV1::Prepare, b"original");
    let native = policy(*admitted.network_id());
    let projection =
        ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&native).unwrap();
    let (_, native_key) = native.issuer_public_key.to_bytes();
    let key: [u8; 32] = native_key.try_into().unwrap();
    assert_eq!(projection.core_preparation_public_key(), key);
    projection
        .require_projected_fields(projection.issuer_policy_digest(), key)
        .unwrap();
    assert!(projection.require_projected_fields([9; 32], key).is_err());
    let app_key: [u8; 32] = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519)
        .public_key()
        .to_bytes()
        .1
        .try_into()
        .unwrap();
    assert!(
        projection
            .require_projected_fields(projection.issuer_policy_digest(), app_key)
            .is_err()
    );
    let mut wrong = native.clone();
    wrong.issuer_public_key = KeyPair::from_seed(vec![7; 32], Algorithm::Secp256k1)
        .public_key()
        .clone();
    assert!(ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&wrong).is_err());
    // A governed issuer using this Ed key in another role is not a blanket policy violation.
    let mut alias_allowed = native;
    alias_allowed.issuer_public_key = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519)
        .public_key()
        .clone();
    let alias_projection =
        ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&alias_allowed)
            .unwrap();
    alias_projection
        .require_projected_fields(alias_projection.issuer_policy_digest(), app_key)
        .unwrap();
}

#[test]
fn issuer_projection_requires_the_retained_native_network_and_exact_auth_namespace() {
    let admitted = verified(ParticipantEnrollmentOperationV1::Prepare, b"original");
    let native = policy(*admitted.network_id());
    let retained =
        RetainedParticipantEnrollmentRequestV1::retain(admitted, b"original".to_vec()).unwrap();
    let dispatch = retained.dispatch().unwrap();
    ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&native)
        .unwrap()
        .require_request_runtime(&dispatch)
        .unwrap();
    let mut other = native.clone();
    other.runtime.authentication_namespace = "hapoalim.is2".parse().unwrap();
    assert!(
        ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&other)
            .unwrap()
            .require_request_runtime(&dispatch)
            .is_err()
    );
    other = native;
    other.runtime.network_id = NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign native network")),
    );
    assert!(
        ParticipantIssuerPreparationProjectionV1::from_native_issuer_policy(&other)
            .unwrap()
            .require_request_runtime(&dispatch)
            .is_err()
    );
}
