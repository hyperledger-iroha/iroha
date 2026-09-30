//! Role15 cannot provision, relabel or enter an ordinary native software adapter.
use super::*;
#[test]
fn final_promotion_account_software_provisioning_rejects_all_handles_and_retains_repair() {
    let parent = temporary_parent();
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let state = parent.path().join("rejected-final-promotion");
    for handle in [
        "software://sorafs/final-promotion-account-transaction/primary",
        "software://sorafs/promotion/primary",
        "hsm://sorafs/final-promotion-account-transaction/primary",
        "kms://sorafs/final-promotion-account-transaction/primary",
        "pkcs11:sorafs/final-promotion-account-transaction/primary",
    ] {
        let mut configured = provisioning(
            SignerRoleV1::FinalPromotionAccountTransaction,
            SignerKeyAlgorithmV1::Ed25519,
        );
        configured.handle = handle.into();
        assert!(configured.purpose_binding.validates_role(configured.role));
        assert!(
            !crate::external_software_signer::protocol::valid_software_signer_handle(
                configured.role,
                handle
            )
        );
        assert!(matches!(
            SoftwareSignerServiceV1::provision(&state, configured, wrapping_key()),
            Err(crate::external_software_signer::SoftwareSignerErrorV1::InvalidBinding)
        ));
        assert!(!state.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    let service = provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let (payload, _) = native_payload(&service);
    let builder = TransactionBuilder::decode_payload(&payload).unwrap();
    assert!(
        crate::external_software_signer::protocol::native_role(SignerRoleV1::Repair).is_some_and(|role| {
            iroha_torii::sorafs::native_transaction_signer::sorafs_native_transaction_payload_matches_role_v1(role, builder.payload())
        })
    );
    assert!(
        !crate::external_software_signer::protocol::native_role(SignerRoleV1::FinalPromotionAccountTransaction).is_some_and(|role| {
            iroha_torii::sorafs::native_transaction_signer::sorafs_native_transaction_payload_matches_role_v1(role, builder.payload())
        })
    );
    assert!(
        crate::external_software_signer::protocol::native_role(
            SignerRoleV1::FinalPromotionAccountTransaction
        )
        .is_none()
    );
    let response = service
        .handle_sign_request(&sign_request(&service, [0x97; 32], payload))
        .unwrap();
    assert_eq!(response.status, SignStatusV1::Ok);
}

#[test]
fn final_promotion_account_cannot_relabel_a_real_software_binding_or_envelope() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let binding = service.public_binding().unwrap();
    binding.validate().unwrap();
    let path = parent.path().join("state/key-envelope-v1.norito");
    let bytes = fs::read(&path).unwrap();
    let envelope: crate::external_software_signer::SoftwareSignerKeyEnvelopeV1 =
        norito::decode_canonical(&bytes).unwrap();
    assert_eq!(
        envelope.open(&wrapping_key()).unwrap().public_key(),
        &binding.public_key
    );
    let mut substituted = binding.clone();
    substituted.role = SignerRoleV1::FinalPromotionAccountTransaction;
    substituted.purpose_binding = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "production-primary".into(),
    };
    substituted.domain = substituted.role.domain().into();
    substituted.handle = "software://sorafs/final-promotion-account-transaction/primary".into();
    assert!(substituted.purpose_binding.validates_role(substituted.role));
    assert!(substituted.validate().is_err());
    assert!(substituted.digest().is_err());
    let mut altered = envelope.clone();
    altered.aad.role = substituted.role;
    altered.aad.purpose_binding = substituted.purpose_binding;
    altered.aad.domain = substituted.domain;
    altered.aad.handle = substituted.handle;
    altered.envelope_digest = altered.compute_digest().unwrap();
    assert_eq!(
        altered.validate_public(),
        Err(crate::external_software_signer::SoftwareSignerEnvelopeErrorV1::Invalid)
    );
    assert!(matches!(
        altered.open(&wrapping_key()),
        Err(crate::external_software_signer::SoftwareSignerEnvelopeErrorV1::Invalid)
    ));
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(service.public_binding().unwrap(), binding);
}

#[test]
fn final_promotion_roles_are_rejected_before_native_adapter_endpoint_access() {
    use crate::external_software_signer::{
        ExternalSoftwareSignerAdapterErrorV1, ExternalSoftwareSignerNativeAdapterV1,
        ExternalSoftwareSignerNativeBackendsV1, SoftwareSignerClientV1,
        SoftwareSignerEndpointPolicyV1,
    };
    use std::sync::Arc;
    let parent = temporary_parent();
    let service = Arc::new(provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    ));
    let original = service.public_binding().unwrap();
    let provenance = service.provenance().unwrap();
    let runtime = parent.path().join("unavailable");
    assert!(!runtime.exists());
    for role in [
        SignerRoleV1::FinalPromotionProvenance,
        SignerRoleV1::FinalPromotionAccountTransaction,
    ] {
        let mut binding = original.clone();
        let intended = provisioning(role, SignerKeyAlgorithmV1::Ed25519);
        binding.role = role;
        binding.purpose_binding = intended.purpose_binding;
        binding.domain = role.domain().into();
        binding.handle = intended.handle;
        assert!(binding.purpose_binding.validates_role(role));
        // Public endpoint fields can be constructed directly; adapter admission itself must
        // reject the role before attempting to qualify any unavailable endpoint.
        let client = SoftwareSignerClientV1::new(SoftwareSignerEndpointPolicyV1 {
            request_socket: runtime.join("request.sock"),
            administrator_socket: runtime.join("administrator.sock"),
            expected_binding: binding,
        });
        assert!(matches!(
            ExternalSoftwareSignerNativeAdapterV1::try_new(client),
            Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch)
        ));
        assert!(!runtime.exists());
        assert_eq!(service.provenance().unwrap(), provenance);
    }
    let client = SoftwareSignerClientV1::new_direct(Arc::clone(&service)).unwrap();
    let adapter = Arc::new(ExternalSoftwareSignerNativeAdapterV1::try_new(client).unwrap());
    let mut backends = ExternalSoftwareSignerNativeBackendsV1::new();
    backends.insert(Arc::clone(&adapter)).unwrap();
    assert!(matches!(
        backends.insert(adapter),
        Err(ExternalSoftwareSignerAdapterErrorV1::RoleMismatch)
    ));
    assert_eq!(service.public_binding().unwrap(), original);
}
