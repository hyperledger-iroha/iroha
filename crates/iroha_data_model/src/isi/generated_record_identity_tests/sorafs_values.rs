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
    // Completion records now carry the governed signer policy directly. Capture
    // actual current Rust values; the retired authority enum has no decoder.
    let owner = crate::account::AccountId::new(
        "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
            .parse()
            .expect("canonical completion owner"),
    );
    let authority = crate::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(
        owner.clone(),
        owner,
        crate::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x91; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0x92; 32],
        },
    );
    assert!(authority.is_valid());
    let provider = crate::sorafs::capacity::ProviderId::new([0x35; 32]);
    let attestation =
        crate::isi::musubi::generated_identity_values::provider_attestation_registration();
    attestation
        .validate()
        .expect("complete signed current attestation");
    vec![
        capacity,
        initializer,
        capture(assertion),
        capture(crate::isi::sorafs::CompleteReplicationOrder::new(
            crate::sorafs::pin_registry::ReplicationOrderId::new([0x44; 32]),
            provider,
            88,
            authority.clone(),
            1,
            crate::sorafs::pin_registry::ProviderIngestFinalizedAnchorV1 {
                height: 87,
                block_hash: [0x93; 32],
            },
        )),
        capture(
            crate::isi::sorafs::SetProviderIngestCompletionAuthority::new(
                provider,
                None,
                authority.clone(),
            ),
        ),
        capture(
            crate::isi::sorafs::RevokeProviderIngestCompletionAuthority::new(provider, authority),
        ),
        capture(attestation),
    ]
}

#[test]
fn current_completion_records_keep_full_governed_and_signed_canonical_frames() {
    use crate::isi::musubi::RegisterMusubiProviderBundleAttestationV1;
    let rows = values();
    assert_eq!(rows.len(), 7);
    let nominals: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|row| row.get("nominal").unwrap().as_str().unwrap())
        .collect();
    assert_eq!(nominals.len(), rows.len());
    for nominal in [
        "iroha_data_model::isi::sorafs::CompleteReplicationOrder",
        "iroha_data_model::isi::sorafs::SetProviderIngestCompletionAuthority",
        "iroha_data_model::isi::sorafs::RevokeProviderIngestCompletionAuthority",
        "iroha_data_model::isi::musubi::RegisterMusubiProviderBundleAttestationV1",
    ] {
        assert!(nominals.contains(nominal));
    }
    let row = rows
        .iter()
        .find(|row| {
            row.get("nominal").unwrap().as_str().unwrap()
                == "iroha_data_model::isi::musubi::RegisterMusubiProviderBundleAttestationV1"
        })
        .unwrap();
    let bytes = hex::decode(row.get("frame").unwrap().as_str().unwrap()).unwrap();
    let mut decoded: RegisterMusubiProviderBundleAttestationV1 =
        norito::decode_from_bytes(&bytes).unwrap();
    decoded.validate().unwrap();
    decoded
        .attestation
        .verify(&decoded.attestation.payload.binding)
        .unwrap();
    assert_eq!(norito::to_bytes(&decoded).unwrap(), bytes);
    decoded
        .attestation
        .payload
        .binding
        .completion_authority
        .signer_policy
        .revision += 1;
    assert!(
        decoded
            .attestation
            .verify(&decoded.attestation.payload.binding)
            .is_err(),
        "changed complete signer binding invalidates its original signature"
    );
}
