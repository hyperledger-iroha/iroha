//! Topology consistency types do not admit a signer without native authority providers.
use super::*;

#[test]
fn topology_cannot_provision_or_relabel_an_ordinary_software_binding() {
    let parent = temporary_parent();
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let state = parent.path().join("rejected-topology");
    for handle in [
        "software://sorafs/topology-approval/primary",
        "software://sorafs/promotion/primary",
        "software://sorafs/repair/primary",
        "hsm://sorafs/topology-approval/primary",
        "kms://sorafs/topology-approval/primary",
    ] {
        let mut configured = provisioning(
            SignerRoleV1::TopologyApproval,
            SignerKeyAlgorithmV1::Ed25519,
        );
        configured.handle = handle.into();
        assert!(configured.purpose_binding.validates_role(configured.role));
        assert!(!super::super::protocol::valid_software_signer_handle(
            configured.role,
            handle
        ));
        assert!(super::super::protocol::native_role(configured.role).is_none());
        assert!(matches!(
            SoftwareSignerServiceV1::provision(&state, configured, wrapping_key()),
            Err(super::super::SoftwareSignerErrorV1::InvalidBinding)
        ));
        assert!(!state.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    let service = provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let mut substituted = service.public_binding().unwrap();
    substituted.role = SignerRoleV1::TopologyApproval;
    substituted.purpose_binding = SignerPurposeBindingV1::TopologyApproval {
        deployment_id: "production-primary".into(),
    };
    substituted.domain = substituted.role.domain().into();
    substituted.handle = "software://sorafs/topology-approval/primary".into();
    assert!(substituted.validate().is_err());
    assert!(substituted.digest().is_err());
    let (payload, _) = native_payload(&service);
    let response = service
        .handle_sign_request(&sign_request(&service, [0x94; 32], payload))
        .unwrap();
    assert_eq!(response.status, SignStatusV1::Ok);
}

#[test]
fn topology_native_adapter_refuses_before_endpoint_access() {
    use crate::external_software_signer::{
        ExternalSoftwareSignerAdapterErrorV1, ExternalSoftwareSignerNativeAdapterV1,
        SoftwareSignerClientV1, SoftwareSignerEndpointPolicyV1,
    };
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SignerRoleV1::Repair,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let before = service.provenance().unwrap();
    let mut binding = service.public_binding().unwrap();
    let intended = provisioning(
        SignerRoleV1::TopologyApproval,
        SignerKeyAlgorithmV1::Ed25519,
    );
    binding.role = intended.role;
    binding.purpose_binding = intended.purpose_binding;
    binding.domain = binding.role.domain().into();
    binding.handle = intended.handle;
    let runtime = parent.path().join("unavailable-topology");
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
    assert_eq!(service.provenance().unwrap(), before);
}
