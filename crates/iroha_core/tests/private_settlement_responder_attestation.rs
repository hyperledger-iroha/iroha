//! Public integration checks for private-settlement responder attestations.

use iroha_core::private_settlement::{
    PrivateSettlementAvailabilityErrorV1, PrivateSettlementAvailabilitySignerV1,
    verify_private_settlement_audit_approval_acknowledgement_attestation_v1,
    verify_private_settlement_auditor_view_attestation_v1,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, DataSpaceId, LaneId,
        PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
        PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
        PrivateSettlementAuditorViewAttestationBodyV1, PrivateSettlementCommitteeAuthorityV1,
        PrivateSettlementRouteV1,
    },
    peer::PeerId,
};

struct ResponderAttestationFixture {
    authority: PrivateSettlementCommitteeAuthorityV1,
    validator_keys: Vec<KeyPair>,
    network_id: NetworkId,
    payload_digest: Hash,
}

fn responder_attestation_fixture() -> ResponderAttestationFixture {
    let mut validator_keys = (0_u8..4)
        .map(|index| KeyPair::from_seed(vec![0xA0 + index; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    validator_keys.sort_by(|left, right| {
        PeerId::from(left.public_key().clone()).cmp(&PeerId::from(right.public_key().clone()))
    });
    let validators = validator_keys
        .iter()
        .map(|key| PeerId::from(key.public_key().clone()))
        .collect::<Vec<_>>();
    let authority = PrivateSettlementCommitteeAuthorityV1 {
        route: PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(7),
            lane_id: LaneId::new(3),
            lane_incarnation: Hash::new(b"private settlement responder lane incarnation"),
        },
        validator_set_hash: HashOf::new(&validators),
        validators,
        validator_pops: validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive validator proof of possession")
            })
            .collect(),
    };
    authority
        .validate()
        .expect("canonical four-validator authority shape");
    for (validator, pop) in authority.validators.iter().zip(&authority.validator_pops) {
        iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop)
            .expect("canonical authority proof of possession");
    }

    ResponderAttestationFixture {
        authority,
        validator_keys,
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"private settlement responder network"),
        )),
        payload_digest: Hash::new(b"private settlement encrypted participant leg"),
    }
}

#[test]
fn public_auditor_view_response_attestation_is_exact_responder_and_signer_bound() {
    let fixture = responder_attestation_fixture();
    let signer = PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
        .expect("canonical roster signer");
    let body = PrivateSettlementAuditorViewAttestationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: fixture.network_id,
        payload_digest: fixture.payload_digest,
        view_digest: Hash::new(b"exact restricted auditor response"),
        authority_digest: fixture.authority.digest().expect("authority digest"),
        lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
        authoritative_height: 11,
        responder: signer.peer_id().clone(),
    };
    let attestation = signer
        .sign_auditor_view(body.clone(), &fixture.authority)
        .expect("sign exact auditor response");
    verify_private_settlement_auditor_view_attestation_v1(&attestation, &body, &fixture.authority)
        .expect("verify exact auditor response");

    let mut substituted_body = body.clone();
    substituted_body.view_digest = Hash::new(b"substituted restricted auditor response");
    assert_eq!(
        verify_private_settlement_auditor_view_attestation_v1(
            &attestation,
            &substituted_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );
    let mut substituted_attestation = attestation.clone();
    substituted_attestation.body = substituted_body.clone();
    assert_eq!(
        verify_private_settlement_auditor_view_attestation_v1(
            &substituted_attestation,
            &substituted_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );

    let mut wrong_responder_body = body.clone();
    wrong_responder_body.responder = fixture.authority.validators[1].clone();
    assert_eq!(
        signer.sign_auditor_view(wrong_responder_body.clone(), &fixture.authority),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner)
    );
    let mut wrong_responder_attestation = attestation.clone();
    wrong_responder_attestation.body = wrong_responder_body.clone();
    assert_eq!(
        verify_private_settlement_auditor_view_attestation_v1(
            &wrong_responder_attestation,
            &wrong_responder_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );

    let mut wrong_signer_attestation = attestation;
    wrong_signer_attestation.signature = Signature::try_new(
        fixture.validator_keys[1].private_key(),
        &body
            .signature_preimage()
            .expect("auditor response preimage"),
    )
    .expect("sign auditor response with wrong roster key")
    .payload()
    .to_vec();
    assert_eq!(
        verify_private_settlement_auditor_view_attestation_v1(
            &wrong_signer_attestation,
            &body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );
}

#[test]
fn public_approval_acknowledgement_response_attestation_is_exact_responder_and_signer_bound() {
    let fixture = responder_attestation_fixture();
    let signer = PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
        .expect("canonical roster signer");
    let body = PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: fixture.network_id,
        payload_digest: fixture.payload_digest,
        approval_digest: Hash::new(b"exact signed auditor approval request"),
        acknowledgement_digest: Hash::new(b"exact durable approval acknowledgement response"),
        authority_digest: fixture.authority.digest().expect("authority digest"),
        lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
        authoritative_height: 11,
        responder: signer.peer_id().clone(),
    };
    let attestation = signer
        .sign_audit_approval_acknowledgement(body.clone(), &fixture.authority)
        .expect("sign exact approval acknowledgement response");
    verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
        &attestation,
        &body,
        &fixture.authority,
    )
    .expect("verify exact approval acknowledgement response");

    let mut substituted_body = body.clone();
    substituted_body.acknowledgement_digest =
        Hash::new(b"substituted durable approval acknowledgement response");
    assert_eq!(
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &attestation,
            &substituted_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );
    let mut substituted_attestation = attestation.clone();
    substituted_attestation.body = substituted_body.clone();
    assert_eq!(
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &substituted_attestation,
            &substituted_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );

    let mut wrong_responder_body = body.clone();
    wrong_responder_body.responder = fixture.authority.validators[1].clone();
    let wrong_responder_result = signer
        .sign_audit_approval_acknowledgement(wrong_responder_body.clone(), &fixture.authority);
    assert_eq!(
        wrong_responder_result,
        Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner)
    );
    let mut wrong_responder_attestation = attestation.clone();
    wrong_responder_attestation.body = wrong_responder_body.clone();
    assert_eq!(
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &wrong_responder_attestation,
            &wrong_responder_body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );

    let mut wrong_signer_attestation = attestation;
    wrong_signer_attestation.signature = Signature::try_new(
        fixture.validator_keys[1].private_key(),
        &body
            .signature_preimage()
            .expect("approval acknowledgement response preimage"),
    )
    .expect("sign approval acknowledgement response with wrong roster key")
    .payload()
    .to_vec();
    assert_eq!(
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &wrong_signer_attestation,
            &body,
            &fixture.authority,
        ),
        Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
    );
}
