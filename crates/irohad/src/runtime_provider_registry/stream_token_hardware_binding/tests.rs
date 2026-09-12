//! Public metadata roundtrips and substitutions; these fixtures do not attest hardware custody.

use super::*;
use crate::runtime_provider_registry::{
    IrohaRuntimeProviderBindingV1, IrohaRuntimeProviderBindingsV1, runtime_provider_test_network_id,
};
use sorafs_manifest::signer::protocol::{
    SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1,
};

#[test]
fn observer_cannot_reuse_the_hardware_client_route() {
    let exact = fixture();
    exact.validate().unwrap();
    assert!(
        StreamTokenHardwareRuntimeBindingV1::new(
            exact.custody().clone(),
            exact.custody().runtime_handle.clone(),
            exact.trust_pins_digest(),
        )
        .is_err()
    );
}

pub(in crate::runtime_provider_registry) fn hardware_config(
    public_key: [u8; 32],
) -> iroha_config::parameters::actual::SorafsStreamTokenHardwareConfig {
    use iroha_config::parameters::actual::{
        SorafsStreamTokenAttesterConfig, SorafsStreamTokenAuthorityConfig,
        SorafsStreamTokenHardwareConfig, SorafsStreamTokenObserverConfig,
    };
    let authority = |seed, service: &str, administrator: &str| {
        let key =
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
                .unwrap();
        SorafsStreamTokenAuthorityConfig {
            service_id: service.to_owned(),
            administrator_id: administrator.to_owned(),
            public_key: key.public_key().to_bytes().1.try_into().unwrap(),
            key_revision: 2,
            policy_revision: 3,
            policy_digest: [seed; 32],
            active_from_unix_ms: 1,
            active_until_unix_ms: u64::MAX,
        }
    };
    SorafsStreamTokenHardwareConfig {
        runtime_handle: "hsm://sorafs/stream-token/primary".to_owned(),
        key_handle: "pkcs11://sorafs/stream-token/key-primary".to_owned(),
        service_id: "stream-signer-primary".to_owned(),
        administrator_id: "stream-signer-admin".to_owned(),
        public_key,
        key_revision: 3,
        policy_revision: 5,
        policy_digest: [0x82; 32],
        attester: SorafsStreamTokenAttesterConfig {
            authority: authority(0x83, "custody-primary", "custody-admin"),
            max_validity_ms: 86_400_000,
            max_anchor_age_ms: 300_000,
        },
        observer: SorafsStreamTokenObserverConfig {
            runtime_handle: "state://sorafs/stream-token/primary".to_owned(),
            authority: authority(0x84, "observer-primary", "observer-admin"),
            max_state_age_ms: 300_000,
        },
    }
}

pub(in crate::runtime_provider_registry) fn fixture() -> StreamTokenHardwareRuntimeBindingV1 {
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    StreamTokenHardwareRuntimeBindingV1::new(
        SignerCustodyBindingV1 {
            chain_id: "hardware-catalog-chain".to_owned(),
            network_id: *runtime_provider_test_network_id().as_bytes(),
            runtime_handle: "hsm://sorafs/stream-token/primary".to_owned(),
            key_handle: "pkcs11://sorafs/stream-token/key-primary".to_owned(),
            service_id: "stream-signer-primary".to_owned(),
            administrator_id: "stream-signer-admin".to_owned(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: [0x72; 32],
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key.public_key().clone(),
            key_revision: 7,
            policy_revision: 11,
            policy_digest: [0x73; 32],
        },
        "state://sorafs/stream-token/primary".to_owned(),
        [0x74; 32],
    )
    .unwrap()
}

#[test]
fn complete_hardware_catalog_survives_canonical_roundtrip_in_every_layout() {
    let hardware = fixture();
    let binding =
        IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(hardware.clone()).unwrap();
    assert_eq!(binding.handle(), hardware.custody().runtime_handle);
    assert_eq!(binding.revision(), Some(hardware.custody().key_revision));
    assert_eq!(
        binding.policy_digest(),
        Some(hardware.custody().policy_digest)
    );
    assert_eq!(binding.stream_token_hardware_binding(), Some(&hardware));
    let catalog = IrohaRuntimeProviderBindingsV1 {
        chain_id: hardware.custody().chain_id.clone(),
        network_id: runtime_provider_test_network_id(),
        bindings: vec![binding],
    };
    let canonical = catalog.export_canonical_v1().unwrap();
    let flags: Vec<_> = (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .collect();
    assert_eq!(flags.len(), 10);
    for flags in flags {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(catalog.export_canonical_v1().unwrap(), canonical);
        let loaded = IrohaRuntimeProviderBindingsV1::load_canonical_v1(&canonical).unwrap();
        assert_eq!(loaded.export_canonical_v1().unwrap(), canonical);
        assert_eq!(
            loaded
                .iter()
                .next()
                .unwrap()
                .stream_token_hardware_binding(),
            Some(&hardware)
        );
    }
}

#[test]
fn structural_metadata_rejects_software_missing_provider_and_unbounded_inputs() {
    let mutations: &[fn(&mut StreamTokenHardwareRuntimeBindingV1)] = &[
        |value| value.custody.runtime_handle = "software://sorafs/stream-token/primary".into(),
        |value| value.custody.key_handle = "software://sorafs/stream-token/key".into(),
        |value| value.custody.role = SignerRoleV1::ReleaseManifest,
        |value| value.custody.purpose = SignerPurposeBindingV1::EvidenceViewer,
        |value| {
            value.custody.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: [0; 32],
            }
        },
        |value| value.custody.algorithm = SignerKeyAlgorithmV1::MlDsa,
        |value| value.custody.key_revision = 0,
        |value| value.custody.key_revision = u64::from(u32::MAX) + 1,
        |value| value.custody.policy_revision = 0,
        |value| value.custody.policy_digest = [0; 32],
        |value| value.custody.network_id = [0; 32],
        |value| value.custody.service_id = value.custody.administrator_id.clone(),
        |value| value.custody.runtime_handle = format!("hsm://{}", "a".repeat(129)),
        |value| value.observer_handle.clear(),
        |value| value.observer_handle = format!("state://{}", "a".repeat(257)),
        |value| value.observer_handle = "software://sorafs/observer/primary".into(),
        |value| value.observer_handle = "state://sorafs/software/primary".into(),
        |value| value.observer_handle = "state://sorafs/test/primary".into(),
        |value| value.observer_handle = "https://operator:credential@observer.invalid".into(),
        |value| value.trust_pins_digest = [0; 32],
    ];
    for mutate in mutations {
        let mut value = fixture();
        mutate(&mut value);
        assert!(value.validate().is_err());
        assert!(IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(value).is_err());
    }
}

#[test]
fn catalog_rejects_enclosing_network_or_header_drift() {
    let hardware = fixture();
    hardware
        .validate_network(&hardware.custody.chain_id, &hardware.custody.network_id)
        .unwrap();
    assert_eq!(
        hardware.validate_network("other-chain", &hardware.custody.network_id),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    );
    assert_eq!(
        hardware.validate_network(&hardware.custody.chain_id, &[0x77; 32]),
        Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
    );
    let binding = IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(hardware).unwrap();
    let mut catalog = IrohaRuntimeProviderBindingsV1 {
        chain_id: "other-chain".to_owned(),
        network_id: runtime_provider_test_network_id(),
        bindings: vec![binding],
    };
    assert!(catalog.export_canonical_v1().is_err());
    catalog.chain_id = "hardware-catalog-chain".to_owned();
    catalog.export_canonical_v1().unwrap();
    let mutations: [fn(&mut IrohaRuntimeProviderBindingV1); 4] = [
        |value| value.handle.push_str("-other"),
        |value| value.revision = Some(8),
        |value| value.policy_digest = Some([0x78; 32]),
        |value| value.stream_token_hardware_binding = None,
    ];
    for mutate in mutations {
        let original = catalog.bindings[0].clone();
        mutate(&mut catalog.bindings[0]);
        assert!(catalog.export_canonical_v1().is_err());
        catalog.bindings[0] = original;
    }
}

#[test]
fn every_provider_or_trust_pin_change_alters_the_exported_catalog_identity() {
    let original = fixture();
    let encode = |hardware: StreamTokenHardwareRuntimeBindingV1| {
        IrohaRuntimeProviderBindingsV1 {
            chain_id: hardware.custody.chain_id.clone(),
            network_id: runtime_provider_test_network_id(),
            bindings: vec![
                IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(hardware).unwrap(),
            ],
        }
        .export_canonical_v1()
        .unwrap()
    };
    let canonical = encode(original.clone());
    let mutations: [fn(&mut StreamTokenHardwareRuntimeBindingV1); 6] = [
        |value| {
            value.custody.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: [0x75; 32],
            }
        },
        |value| value.custody.key_handle.push_str("-next"),
        |value| value.custody.policy_revision += 1,
        |value| value.custody.administrator_id.push_str("-other"),
        |value| value.observer_handle.push_str("-other"),
        |value| value.trust_pins_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert_ne!(encode(changed), canonical);
    }
}
