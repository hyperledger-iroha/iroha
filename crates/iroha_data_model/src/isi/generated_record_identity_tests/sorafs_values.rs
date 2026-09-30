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
