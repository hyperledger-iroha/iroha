//! Stream-token software entry-point rejection with real temporary service state.

use super::*;
use crate::external_software_signer::{
    SoftwareSignerEnvelopeErrorV1, SoftwareSignerErrorV1, SoftwareSignerKeyEnvelopeV1,
    protocol::valid_software_signer_handle,
};

#[test]
fn stream_token_software_provisioning_rejects_before_creating_state() {
    let parent = temporary_parent();
    fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let state = parent.path().join("rejected-stream-state");
    for handle in [
        "software://sorafs/stream-token/primary",
        "software://sorafs/evidence-viewer/primary",
        "hsm://sorafs/stream-token/primary",
        "kms://sorafs/stream-token/primary",
        "pkcs11:sorafs/stream-token/primary",
    ] {
        let mut configured = provisioning(SignerRoleV1::StreamToken, SignerKeyAlgorithmV1::Ed25519);
        configured.handle = handle.into();
        assert!(configured.purpose_binding.validates_role(configured.role));
        assert!(!valid_software_signer_handle(configured.role, handle));
        assert!(matches!(
            SoftwareSignerServiceV1::provision(&state, configured, wrapping_key()),
            Err(SoftwareSignerErrorV1::InvalidBinding)
        ));
        assert!(!state.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    // The same entry point still provisions and reopens an allowed role.
    let service = provision(
        parent.path(),
        SignerRoleV1::EvidenceViewer,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let binding = service.public_binding().unwrap();
    drop(service);
    assert_eq!(
        SoftwareSignerServiceV1::open(parent.path().join("state"), wrapping_key())
            .unwrap()
            .public_binding()
            .unwrap(),
        binding
    );
}

#[test]
fn stream_token_software_public_binding_and_envelope_fail_before_unwrap() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SignerRoleV1::EvidenceViewer,
        SignerKeyAlgorithmV1::Ed25519,
    );
    let binding = service.public_binding().unwrap();
    assert!(binding.validate().is_ok());
    let path = parent.path().join("state/key-envelope-v1.norito");
    let bytes = fs::read(&path).unwrap();
    let envelope: SoftwareSignerKeyEnvelopeV1 = norito::decode_canonical(&bytes).unwrap();
    envelope.validate_public().unwrap();
    assert_eq!(
        envelope.open(&wrapping_key()).unwrap().public_key(),
        &binding.public_key
    );
    for handle in [
        "software://sorafs/stream-token/primary",
        "hsm://sorafs/stream-token/primary",
    ] {
        let mut substituted = binding.clone();
        substituted.role = SignerRoleV1::StreamToken;
        substituted.purpose_binding = SignerPurposeBindingV1::StreamToken {
            provider_id: [0x62; 32],
        };
        substituted.domain = SignerRoleV1::StreamToken.domain().into();
        substituted.handle = handle.into();
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
            Err(SoftwareSignerEnvelopeErrorV1::Invalid)
        );
        for key in [
            wrapping_key(),
            SoftwareSignerWrappingKeyV1::try_from_bytes([0x92; 32]).unwrap(),
        ] {
            assert!(matches!(
                altered.open(&key),
                Err(SoftwareSignerEnvelopeErrorV1::Invalid)
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    assert_eq!(service.public_binding().unwrap(), binding);
}
