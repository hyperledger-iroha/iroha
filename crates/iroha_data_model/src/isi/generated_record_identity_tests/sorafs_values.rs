//! Existing populated `SoraFS` values shared by explicit native maintenance captures.

use super::super::{capture, json::Value};

pub(super) fn values() -> Vec<Value> {
    let row = capture(crate::isi::sorafs::RegisterCapacityDeclaration::new(vec![
        1, 2, 3,
    ]));
    let capacity = row;
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x51; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("capture council/owner key");
    let envelope: sorafs_manifest::ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
        )))
        .expect("structural provider material fixture");
    let material = sorafs_manifest::provider_admission::ProviderAdmissionGenesisMaterialV1 {
        proposal: envelope.proposal,
        advert_body: envelope.advert_body,
        issued_at: envelope.issued_at,
        retention_epoch: envelope.retention_epoch,
    };
    material
        .validate()
        .expect("valid network-independent material");
    let initializer = crate::isi::sorafs::InitializeSorafsProviderAdmissionV1 {
        council: crate::sorafs::provider_admission::governance::InitialProviderAdmissionCouncilV1 {
            policy_id: [0x52; 32],
            trusted_signers: vec![key.public_key().to_bytes().1.try_into().expect("Ed25519")],
            signature_threshold: 1,
        },
        providers: vec![
            crate::sorafs::provider_admission::governance::InitialProviderAdmissionV1 {
                owner: crate::account::AccountId::new(key.public_key().clone()),
                material: norito::encode_canonical(&material).expect("canonical genesis material"),
            },
        ],
    };
    let initializer = capture(initializer);
    let assertion = crate::isi::sorafs::AssertSorafsPublicationV1 {
        manifest_digest: crate::sorafs::pin_registry::ManifestDigest::new([0x61; 32]),
        order_id: crate::sorafs::pin_registry::ReplicationOrderId::new([0x62; 32]),
        assignment_revision: 3,
        canonical_order_digest: [0x63; 32],
        require_complete: true,
        challenge: [0x64; 32],
        minimum_height: 9,
        minimum_block_hash: [0x65; 32],
    };
    vec![capacity, initializer, capture(assertion)]
}

/// Capture the three instructions containing the current governed completion signer.
pub(super) fn completion_authority_values() -> Vec<Value> {
    use crate::{
        account::AccountId,
        isi::sorafs::{
            CompleteReplicationOrder, RevokeProviderIngestCompletionAuthority,
            SetProviderIngestCompletionAuthority,
        },
        sorafs::{
            capacity::ProviderId,
            pin_registry::{
                ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
                ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
            },
        },
    };
    let account = |seed| {
        AccountId::new(
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("capture completion authority key")
                .public_key()
                .clone(),
        )
    };
    let authority = ProviderIngestCompletionAuthorityV1::new(
        account(0x51),
        account(0x52),
        ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x91; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0x92; 32],
        },
    );
    assert!(authority.is_valid());
    assert_ne!(authority.provider_owner, authority.completion_signer);
    let provider = ProviderId::new([0x35; 32]);
    vec![
        capture(CompleteReplicationOrder::new(
            ReplicationOrderId::new([0x44; 32]),
            provider,
            88,
            authority.clone(),
            1,
            ProviderIngestFinalizedAnchorV1 {
                height: 87,
                block_hash: [0x93; 32],
            },
        )),
        capture(SetProviderIngestCompletionAuthority::new(
            provider,
            None,
            authority.clone(),
        )),
        capture(RevokeProviderIngestCompletionAuthority::new(
            provider, authority,
        )),
    ]
}

#[test]
fn completion_instruction_captures_keep_owner_and_signer_distinct() {
    use crate::isi::sorafs::{
        CompleteReplicationOrder, RevokeProviderIngestCompletionAuthority,
        SetProviderIngestCompletionAuthority,
    };
    fn decode<T: for<'de> norito::NoritoDeserialize<'de>>(row: &Value) -> T {
        let frame = hex::decode(row.get("frame").and_then(Value::as_str).unwrap()).unwrap();
        norito::decode_from_bytes(&frame).unwrap()
    }
    let rows = completion_authority_values();
    assert_eq!(rows.len(), 3);
    let complete: CompleteReplicationOrder = decode(&rows[0]);
    let set: SetProviderIngestCompletionAuthority = decode(&rows[1]);
    let revoke: RevokeProviderIngestCompletionAuthority = decode(&rows[2]);
    assert!(complete.expected_authority.is_valid());
    assert_ne!(
        complete.expected_authority.provider_owner,
        complete.expected_authority.completion_signer
    );
    assert_eq!(complete.expected_authority, set.next);
    assert_eq!(complete.expected_authority, revoke.expected_current);
    assert_eq!(complete.provider_id, set.provider_id);
    assert_eq!(complete.provider_id, revoke.provider_id);
    assert!(set.expected_current.is_none());
    assert!(complete.finalized_anchor.is_valid());
}
