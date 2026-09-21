//! Shared native-role authorization at software client, service and adapter boundaries.

use super::*;
use crate::external_software_signer::{
    ExternalSoftwareSignerNativeAdapterV1, SoftwareSignerClientV1, SoftwareSignerEndpointPolicyV1,
    unix::ExternalSoftwareSignerClientErrorV1,
};
use iroha_data_model::transaction::TransactionPayload;
use iroha_torii::{SoraFsRepairTransactionSigner, SoraFsRepairTransactionSigningError};
use std::sync::Arc;

fn repair_and_cross_role_payloads(
    service: &SoftwareSignerServiceV1,
) -> (TransactionPayload, TransactionPayload) {
    let (encoded, _) = native_payload(service);
    let repair = TransactionBuilder::decode_payload(&encoded).unwrap();
    let orderbook = TransactionBuilder::new(
        test_network_id(),
        repair.payload().authority().clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([MatchSorafsOrderbook::new([0x71; 32], 1, 1)]);
    (repair.payload().clone(), orderbook.payload().clone())
}

#[test]
fn native_client_rejects_cross_role_before_accessing_an_unavailable_endpoint() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let (repair, orderbook) = repair_and_cross_role_payloads(&service);
    let runtime = parent.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o711)).unwrap();
    let policy = SoftwareSignerEndpointPolicyV1::try_new(
        runtime.join("request.sock"),
        runtime.join("administrator.sock"),
        service.public_binding().unwrap(),
    )
    .unwrap();
    let client = SoftwareSignerClientV1::new(policy);
    for payload in [
        TransactionBuilder::from_payload(orderbook)
            .unwrap()
            .encode_payload(),
        vec![0x01],
    ] {
        assert_eq!(
            client.sign([0x72; 32], &payload),
            Err(ExternalSoftwareSignerClientErrorV1::Rejected)
        );
    }
    // The valid role reaches endpoint lookup, proving the malformed cases returned earlier.
    assert_eq!(
        client.sign(
            [0x73; 32],
            &TransactionBuilder::from_payload(repair)
                .unwrap()
                .encode_payload(),
        ),
        Err(ExternalSoftwareSignerClientErrorV1::Unavailable)
    );
    assert_eq!(fs::read_dir(runtime).unwrap().count(), 0);
}

#[test]
fn native_adapter_rejects_cross_role_before_a_revoked_service_and_preserves_valid_signing() {
    let parent = temporary_parent();
    let service = Arc::new(provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    ));
    let (repair, orderbook) = repair_and_cross_role_payloads(&service);
    let client = SoftwareSignerClientV1::new_direct(Arc::clone(&service)).unwrap();
    let adapter = ExternalSoftwareSignerNativeAdapterV1::try_new(client).unwrap();
    let signed = SoraFsRepairTransactionSigner::sign(&adapter, repair.clone()).unwrap();
    assert_eq!(signed.payload(), &repair);
    signed.verify_signature().unwrap();
    let before = service.provenance().unwrap();
    let rejected = service
        .handle_sign_request(&sign_request(
            &service,
            [0x74; 32],
            TransactionBuilder::from_payload(orderbook.clone())
                .unwrap()
                .encode_payload(),
        ))
        .unwrap();
    assert_eq!(rejected.status, SignStatusV1::Rejected);
    assert!(service.provenance().unwrap().has_same_stable_state(&before));
    let revoked = service
        .handle_admin_request(&admin_request(
            &service,
            AdminCommandV1::Revoke {
                operation_id: [0x75; 32],
                expected_audit_head: before.audit_head,
                expected_key_revision: before.binding.key_revision,
                reason_digest: [0x76; 32],
            },
        ))
        .unwrap();
    assert!(revoked.provenance.revoked);
    assert_eq!(
        SoraFsRepairTransactionSigner::sign(&adapter, orderbook),
        Err(SoraFsRepairTransactionSigningError::Refused)
    );
    assert_eq!(
        SoraFsRepairTransactionSigner::sign(&adapter, repair),
        Err(SoraFsRepairTransactionSigningError::QualificationChanged)
    );
    assert!(
        service
            .provenance()
            .unwrap()
            .has_same_stable_state(&revoked.provenance)
    );
}

#[test]
fn software_provisioning_binding_and_envelope_preserve_real_identity_words() {
    let parent = temporary_parent();
    let mut configured = provisioning(SignerRoleV1::Repair, SignerKeyAlgorithmV1::Ed25519);
    configured.service_id = "Account-Attester".into();
    configured.administrator_id = "Attestation-Security".into();
    configured.handle = "software://sorafs/repair/latest-contest".into();
    let service =
        SoftwareSignerServiceV1::provision(parent.path().join("state"), configured, wrapping_key())
            .unwrap();
    let binding = service.public_binding().unwrap();
    binding.validate().unwrap();
    assert_eq!(binding.service_id, "Account-Attester");
    assert_eq!(binding.administrator_id, "Attestation-Security");
    assert_eq!(binding.handle, "software://sorafs/repair/latest-contest");
    let bytes = fs::read(parent.path().join("state/key-envelope-v1.norito")).unwrap();
    let envelope: crate::external_software_signer::SoftwareSignerKeyEnvelopeV1 =
        norito::decode_canonical(&bytes).unwrap();
    envelope.aad.validate().unwrap();
    assert_eq!(
        envelope.open(&wrapping_key()).unwrap().public_key(),
        &binding.public_key
    );
    let mut changed = binding.clone();
    changed.service_id = "account-attester".into();
    assert_ne!(changed.digest().unwrap(), binding.digest().unwrap());
    for reserved in [
        "null",
        "mock",
        "test",
        "dev",
        "demo",
        "fake",
        "dummy",
        "placeholder",
    ] {
        for administrator in [false, true] {
            let marked = format!("production-{}-primary", reserved.to_ascii_uppercase());
            let mut binding = binding.clone();
            let mut aad = envelope.aad.clone();
            if administrator {
                binding.administrator_id = marked.clone();
                aad.administrator_id = marked;
            } else {
                binding.service_id = marked.clone();
                aad.service_id = marked;
            }
            assert!(binding.validate().is_err());
            assert!(aad.validate().is_err());
        }
    }
}

#[test]
fn software_provisioning_rejects_reserved_components_before_creating_state() {
    let parent = temporary_parent();
    let state = parent.path().join("state");
    for reserved in [
        "null",
        "mock",
        "test",
        "dev",
        "demo",
        "fake",
        "dummy",
        "placeholder",
    ] {
        for field in 0..3 {
            let mut configured = provisioning(SignerRoleV1::Repair, SignerKeyAlgorithmV1::Ed25519);
            let marked = format!("production-{}-primary", reserved.to_ascii_uppercase());
            match field {
                0 => configured.service_id = marked,
                1 => configured.administrator_id = marked,
                2 => configured.handle = format!("software://sorafs/repair/{marked}"),
                _ => unreachable!(),
            }
            assert!(matches!(
                SoftwareSignerServiceV1::provision(&state, configured, wrapping_key()),
                Err(crate::external_software_signer::SoftwareSignerErrorV1::InvalidBinding)
            ));
            assert!(!state.exists());
            assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
        }
    }
}
